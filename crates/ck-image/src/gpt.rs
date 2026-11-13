use std::fmt;
use std::io::{Read, Seek, SeekFrom};

use crate::error::Error;

/// Image files use 512-byte sectors.
pub const SECTOR_SIZE: u64 = 512;
const SECTOR_BYTES: usize = 512;

const SIGNATURE: &[u8; 8] = b"EFI PART";
const MIN_HEADER_SIZE: usize = 92;
const MAX_ENTRIES: u32 = 1024;
const MIN_ENTRY_SIZE: u32 = 128;
/// The standard size is 128; the format allows larger powers of two, but nothing real needs more.
const MAX_ENTRY_SIZE: u32 = 4096;

/// A partition type id, kept as stored on disk (the first three fields are little-endian).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Guid([u8; 16]);

impl Guid {
    /// C12A7328-F81F-11D2-BA4B-00A0C93EC93B
    pub const EFI_SYSTEM: Self = Self([
        0x28, 0x73, 0x2A, 0xC1, 0x1F, 0xF8, 0xD2, 0x11, 0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E, 0xC9,
        0x3B,
    ]);
    /// 0FC63DAF-8483-4772-8E79-3D69D8477DE4
    pub const LINUX_FILESYSTEM: Self = Self([
        0xAF, 0x3D, 0xC6, 0x0F, 0x83, 0x84, 0x72, 0x47, 0x8E, 0x79, 0x3D, 0x69, 0xD8, 0x47, 0x7D,
        0xE4,
    ]);

    #[must_use]
    pub fn from_disk_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn to_disk_bytes(self) -> [u8; 16] {
        self.0
    }

    fn is_zero(self) -> bool {
        self.0 == [0; 16]
    }
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let b = self.0;
        write!(
            f,
            "{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            b[3],
            b[2],
            b[1],
            b[0],
            b[5],
            b[4],
            b[7],
            b[6],
            b[8],
            b[9],
            b[10],
            b[11],
            b[12],
            b[13],
            b[14],
            b[15]
        )
    }
}

impl fmt::Debug for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Guid({self})")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    pub type_guid: Guid,
    pub first_sector: u64,
    pub last_sector: u64,
    pub name: String,
}

impl Partition {
    #[must_use]
    pub fn start_byte(&self) -> u64 {
        self.first_sector * SECTOR_SIZE
    }

    #[must_use]
    pub fn end_byte(&self) -> u64 {
        (self.last_sector + 1) * SECTOR_SIZE
    }
}

/// Reads the partitions of a GPT disk. Only the primary table is read, and its checksums are verified.
pub fn read_partitions<R: Read + Seek>(disk: &mut R) -> Result<Vec<Partition>, Error> {
    let mut header = [0_u8; SECTOR_BYTES];
    read_at(disk, SECTOR_SIZE, &mut header)?;
    if &header[..8] != SIGNATURE {
        return Err(Error::NoPartitionTable);
    }

    let header_size = le_u32(&header, 12) as usize;
    if !(MIN_HEADER_SIZE..=header.len()).contains(&header_size) {
        return Err(Error::BadPartitionTable("the header has a wrong size"));
    }
    let stored_crc = le_u32(&header, 16);
    let mut checked = header[..header_size].to_vec();
    checked[16..20].fill(0);
    if crc32fast::hash(&checked) != stored_crc {
        return Err(Error::BadPartitionTable(
            "the header checksum does not match",
        ));
    }

    let entries_lba = le_u64(&header, 72);
    let count = le_u32(&header, 80);
    let entry_size = le_u32(&header, 84);
    if count > MAX_ENTRIES || !(MIN_ENTRY_SIZE..=MAX_ENTRY_SIZE).contains(&entry_size) {
        return Err(Error::BadPartitionTable(
            "the partition entries have an unexpected size",
        ));
    }
    let entry_size = entry_size as usize;
    let entries_offset = entries_lba
        .checked_mul(SECTOR_SIZE)
        .ok_or(Error::BadPartitionTable(
            "the partition list is placed outside the disk",
        ))?;
    let mut entries = vec![0_u8; count as usize * entry_size];
    read_at(disk, entries_offset, &mut entries)?;
    if crc32fast::hash(&entries) != le_u32(&header, 88) {
        return Err(Error::BadPartitionTable(
            "the partition list checksum does not match",
        ));
    }

    let mut partitions = Vec::new();
    for entry in entries.chunks_exact(entry_size) {
        if let Some(partition) = parse_entry(entry)? {
            partitions.push(partition);
        }
    }
    Ok(partitions)
}

/// `None` for an unused slot.
fn parse_entry(entry: &[u8]) -> Result<Option<Partition>, Error> {
    let mut guid = [0_u8; 16];
    guid.copy_from_slice(&entry[..16]);
    let type_guid = Guid::from_disk_bytes(guid);
    if type_guid.is_zero() {
        return Ok(None);
    }

    let first_sector = le_u64(entry, 32);
    let last_sector = le_u64(entry, 40);
    // Checked here so that `start_byte` and `end_byte` can never overflow.
    let end_byte = last_sector
        .checked_add(1)
        .and_then(|end| end.checked_mul(SECTOR_SIZE));
    if first_sector > last_sector || end_byte.is_none() {
        return Err(Error::BadPartitionTable(
            "a partition has impossible start and end sectors",
        ));
    }
    Ok(Some(Partition {
        type_guid,
        first_sector,
        last_sector,
        name: utf16_name(&entry[56..128]),
    }))
}

/// The EFI system partition, where the settings go.
pub fn find_efi(partitions: &[Partition]) -> Result<&Partition, Error> {
    partitions
        .iter()
        .find(|p| p.type_guid == Guid::EFI_SYSTEM)
        .ok_or(Error::NoEfiPartition)
}

/// The first Linux filesystem partition, the root of the contest system.
pub fn find_linux_root(partitions: &[Partition]) -> Result<&Partition, Error> {
    partitions
        .iter()
        .find(|p| p.type_guid == Guid::LINUX_FILESYSTEM)
        .ok_or(Error::NoLinuxPartition)
}

fn read_at<R: Read + Seek>(disk: &mut R, offset: u64, buffer: &mut [u8]) -> Result<(), Error> {
    disk.seek(SeekFrom::Start(offset))
        .and_then(|_| disk.read_exact(buffer))
        .map_err(|source| match source.kind() {
            std::io::ErrorKind::UnexpectedEof => Error::NoPartitionTable,
            _ => Error::io("reading the partition table", source),
        })
}

fn le_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn le_u64(bytes: &[u8], at: usize) -> u64 {
    let mut value = [0_u8; 8];
    value.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(value)
}

fn utf16_name(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|&unit| unit != 0)
        .collect();
    String::from_utf16_lossy(&units)
}
