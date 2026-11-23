//! The `CKCONFIG` partition: finding it, reading and writing files in it, and the end of the part of
//! the image that can differ from stick to stick.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use ck_image::{
    Error, Guid, SECTOR_SIZE, find_config, has_config_partition, read_config_file, read_partitions,
    settings_end, write_config_file,
};
use fatfs::{FatType, FormatVolumeOptions};
use fscommon::StreamSlice;
use tempfile::TempDir;

const MIB: u64 = 1024 * 1024;
const SECTORS_PER_MIB: u64 = MIB / SECTOR_SIZE;
const IMAGE_SIZE: u64 = 128 * MIB;
const EFI_FIRST: u64 = 2048;
const EFI_LAST: u64 = EFI_FIRST + 48 * SECTORS_PER_MIB - 1;
const CONFIG_FIRST: u64 = EFI_LAST + 1;
const CONFIG_LAST: u64 = CONFIG_FIRST + 16 * SECTORS_PER_MIB - 1;
const ROOT_FIRST: u64 = CONFIG_LAST + 1;
const ROOT_LAST: u64 = IMAGE_SIZE / SECTOR_SIZE - 34;

struct Entry {
    guid: Guid,
    first: u64,
    last: u64,
    name: &'static str,
}

fn entries(config_name: &'static str) -> Vec<Entry> {
    vec![
        Entry {
            guid: Guid::EFI_SYSTEM,
            first: EFI_FIRST,
            last: EFI_LAST,
            name: "EFI",
        },
        Entry {
            guid: Guid::BASIC_DATA,
            first: CONFIG_FIRST,
            last: CONFIG_LAST,
            name: config_name,
        },
        Entry {
            guid: Guid::LINUX_FILESYSTEM,
            first: ROOT_FIRST,
            last: ROOT_LAST,
            name: "root",
        },
    ]
}

fn make_image(dir: &TempDir, partitions: &[Entry]) -> PathBuf {
    let path = dir.path().join("test.img");
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .expect("create image");
    file.set_len(IMAGE_SIZE).expect("size image");
    write_gpt(&mut file, partitions);
    file.seek(SeekFrom::Start(ROOT_FIRST * SECTOR_SIZE))
        .expect("seek root");
    file.write_all(&[0xAB; 4096]).expect("fill root");

    for (first, last, label, fat) in [
        (EFI_FIRST, EFI_LAST, *b"EFI        ", FatType::Fat32),
        (CONFIG_FIRST, CONFIG_LAST, *b"CKCONFIG   ", FatType::Fat16),
    ] {
        let slice = StreamSlice::new(
            file.try_clone().expect("clone"),
            first * SECTOR_SIZE,
            (last + 1) * SECTOR_SIZE,
        )
        .expect("slice");
        let options = FormatVolumeOptions::new().fat_type(fat).volume_label(label);
        fatfs::format_volume(slice, options).expect("format");
    }
    path
}

fn write_gpt(file: &mut File, partitions: &[Entry]) {
    let mut table = vec![0_u8; 128 * 128];
    for (index, p) in partitions.iter().enumerate() {
        let entry = &mut table[index * 128..(index + 1) * 128];
        entry[..16].copy_from_slice(&p.guid.to_disk_bytes());
        entry[16..32].copy_from_slice(&[u8::try_from(index + 1).expect("small"); 16]);
        entry[32..40].copy_from_slice(&p.first.to_le_bytes());
        entry[40..48].copy_from_slice(&p.last.to_le_bytes());
        for (i, unit) in p.name.encode_utf16().enumerate() {
            entry[56 + i * 2..58 + i * 2].copy_from_slice(&unit.to_le_bytes());
        }
    }
    let mut header = [0_u8; 92];
    header[..8].copy_from_slice(b"EFI PART");
    header[8..12].copy_from_slice(&0x0001_0000_u32.to_le_bytes());
    header[12..16].copy_from_slice(&92_u32.to_le_bytes());
    header[24..32].copy_from_slice(&1_u64.to_le_bytes());
    header[72..80].copy_from_slice(&2_u64.to_le_bytes());
    header[80..84].copy_from_slice(&128_u32.to_le_bytes());
    header[84..88].copy_from_slice(&128_u32.to_le_bytes());
    header[88..92].copy_from_slice(&crc32fast::hash(&table).to_le_bytes());
    let crc = crc32fast::hash(&header);
    header[16..20].copy_from_slice(&crc.to_le_bytes());
    file.seek(SeekFrom::Start(SECTOR_SIZE)).expect("seek");
    file.write_all(&header).expect("header");
    file.seek(SeekFrom::Start(2 * SECTOR_SIZE)).expect("seek");
    file.write_all(&table).expect("entries");
}

fn partitions_of(image: &Path) -> Vec<ck_image::Partition> {
    read_partitions(&mut File::open(image).expect("open")).expect("partitions")
}

fn size(n: u64) -> usize {
    usize::try_from(n).expect("fits")
}

fn bytes(path: &Path, start: u64, length: usize) -> Vec<u8> {
    let mut file = File::open(path).expect("open");
    file.seek(SeekFrom::Start(start)).expect("seek");
    let mut out = vec![0; length];
    file.read_exact(&mut out).expect("read");
    out
}

#[test]
fn the_config_partition_is_found_by_its_name() {
    let dir = TempDir::new().expect("temp dir");
    let image = make_image(&dir, &entries("CKCONFIG"));
    let partitions = partitions_of(&image);
    let found = find_config(&partitions).expect("found");
    assert_eq!(found.first_sector, CONFIG_FIRST);
    assert!(has_config_partition(&image).expect("checked"));
}

#[test]
fn an_unnamed_config_partition_is_found_by_its_type_and_place() {
    let dir = TempDir::new().expect("temp dir");
    let image = make_image(&dir, &entries(""));
    let partitions = partitions_of(&image);
    let found = find_config(&partitions).expect("found");
    assert_eq!(found.first_sector, CONFIG_FIRST);
}

#[test]
fn an_image_without_the_partition_has_none_and_its_head_ends_with_the_efi_partition() {
    let dir = TempDir::new().expect("temp dir");
    let mut table = entries("CKCONFIG");
    table.remove(1);
    let image = make_image(&dir, &table);
    assert!(!has_config_partition(&image).expect("checked"));
    assert_eq!(
        settings_end(&partitions_of(&image)).expect("end"),
        (EFI_LAST + 1) * SECTOR_SIZE
    );
    assert!(matches!(
        write_config_file(&image, "config.json", b"{}"),
        Err(Error::NoConfigPartition)
    ));
}

#[test]
fn the_head_of_the_image_reaches_the_end_of_the_config_partition() {
    let dir = TempDir::new().expect("temp dir");
    let image = make_image(&dir, &entries("CKCONFIG"));
    assert_eq!(
        settings_end(&partitions_of(&image)).expect("end"),
        (CONFIG_LAST + 1) * SECTOR_SIZE
    );
}

#[test]
fn a_config_partition_after_the_root_is_refused() {
    let dir = TempDir::new().expect("temp dir");
    let mut table = entries("CKCONFIG");
    // The Linux root takes the middle of the disk and the config partition comes after it.
    (table[1].first, table[1].last) = (ROOT_FIRST, ROOT_LAST);
    (table[2].first, table[2].last) = (CONFIG_FIRST, CONFIG_LAST);
    let image = make_image(&dir, &table);
    assert!(settings_end(&partitions_of(&image)).is_err());
}

#[test]
fn a_file_written_to_the_config_partition_reads_back_and_nothing_else_changes() {
    let dir = TempDir::new().expect("temp dir");
    let image = make_image(&dir, &entries("CKCONFIG"));
    let before_efi = bytes(&image, 0, size(EFI_FIRST * SECTOR_SIZE));
    let efi = bytes(&image, EFI_FIRST * SECTOR_SIZE, size(48 * MIB));
    let root = bytes(&image, ROOT_FIRST * SECTOR_SIZE, 4096);

    write_config_file(&image, "config.json", br#"{"a":1}"#).expect("write");
    write_config_file(&image, "config.json", br#"{"a":2}"#).expect("replace");
    write_config_file(&image, "wall.png", b"picture").expect("wallpaper");

    assert_eq!(
        read_config_file(&image, "config.json").expect("read"),
        br#"{"a":2}"#
    );
    assert_eq!(
        read_config_file(&image, "wall.png").expect("read"),
        b"picture"
    );
    assert_eq!(bytes(&image, 0, before_efi.len()), before_efi);
    assert_eq!(
        bytes(&image, EFI_FIRST * SECTOR_SIZE, efi.len()),
        efi,
        "the EFI partition"
    );
    assert_eq!(bytes(&image, ROOT_FIRST * SECTOR_SIZE, 4096), root);
}

#[test]
fn a_missing_file_and_a_path_with_folders_are_errors() {
    let dir = TempDir::new().expect("temp dir");
    let image = make_image(&dir, &entries("CKCONFIG"));
    assert!(matches!(
        read_config_file(&image, "nothing.json"),
        Err(Error::FileNotFound(_))
    ));
    for bad in ["a/b.json", "a\\b.json", "", "..", "."] {
        assert!(
            matches!(write_config_file(&image, bad, b"x"), Err(Error::BadPath(_))),
            "{bad:?}"
        );
    }
}
