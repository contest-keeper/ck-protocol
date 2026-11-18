//! Repairs the `.` and `..` entries that the `fatfs` crate writes for new directories.
//!
//! `fatfs` 0.3 puts a long-name entry in front of both entries, so a directory starts with
//! `LFN, ".", LFN, ".."`. Linux `fsck.fat` calls that damage and rewrites the directory at every
//! boot ("Expected a valid '.' entry in this slot"). A correct directory starts with `.` and `..`
//! directly, and `..` points at cluster 0 when the parent is the root.
//!
//! Only FAT32 is handled, which is what the EFI partition is. Other volumes are left alone.

use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

use crate::error::Error;

const ENTRY: usize = 32;
const ATTR_LONG_NAME: u8 = 0x0F;
const ATTR_DIRECTORY: u8 = 0x10;
const DELETED: u8 = 0xE5;
const END_OF_CHAIN: u32 = 0x0FFF_FFF8;
const DOT: &[u8; 11] = b".          ";
const DOT_DOT: &[u8; 11] = b"..         ";
/// A directory with more clusters than this is not one we made, so it is left alone.
const MAX_DIRECTORY_CLUSTERS: usize = 256;
const MAX_DEPTH: usize = 16;

/// Fixes every directory of the FAT32 volume that starts at byte `base` of `file`.
pub(crate) fn repair(file: &mut File, base: u64) -> Result<(), Error> {
    let Some(volume) = Volume::read(file, base)? else {
        return Ok(());
    };
    let mut seen = HashSet::new();
    repair_directory(file, &volume, volume.root, 0, 0, &mut seen)
}

struct Volume {
    cluster_bytes: u64,
    fat_start: u64,
    data_start: u64,
    root: u32,
}

impl Volume {
    fn read(file: &mut File, base: u64) -> Result<Option<Self>, Error> {
        let mut boot = [0_u8; 64];
        seek(file, base)?;
        if file.read_exact(&mut boot).is_err() {
            return Ok(None);
        }
        let bytes_per_sector = u64::from(u16::from_le_bytes([boot[11], boot[12]]));
        let sectors_per_cluster = u64::from(boot[13]);
        let reserved = u64::from(u16::from_le_bytes([boot[14], boot[15]]));
        let fats = u64::from(boot[16]);
        let fat_size = u64::from(u32::from_le_bytes([boot[36], boot[37], boot[38], boot[39]]));
        let root = u32::from_le_bytes([boot[44], boot[45], boot[46], boot[47]]);
        // FAT12 and FAT16 have no 32-bit FAT size.
        if fat_size == 0 || bytes_per_sector < 512 || sectors_per_cluster == 0 || root < 2 {
            return Ok(None);
        }
        Ok(Some(Self {
            cluster_bytes: bytes_per_sector * sectors_per_cluster,
            fat_start: base + reserved * bytes_per_sector,
            data_start: base + (reserved + fats * fat_size) * bytes_per_sector,
            root,
        }))
    }

    fn cluster_offset(&self, cluster: u32) -> u64 {
        self.data_start + u64::from(cluster - 2) * self.cluster_bytes
    }

    fn next(&self, file: &mut File, cluster: u32) -> Result<u32, Error> {
        seek(file, self.fat_start + u64::from(cluster) * 4)?;
        let mut raw = [0_u8; 4];
        file.read_exact(&mut raw)
            .map_err(|e| Error::io("reading the FAT", e))?;
        Ok(u32::from_le_bytes(raw) & 0x0FFF_FFFF)
    }

    fn chain(&self, file: &mut File, first: u32) -> Result<Vec<u32>, Error> {
        let mut clusters = Vec::new();
        let mut cluster = first;
        while (2..END_OF_CHAIN).contains(&cluster) && clusters.len() <= MAX_DIRECTORY_CLUSTERS {
            clusters.push(cluster);
            cluster = self.next(file, cluster)?;
        }
        Ok(clusters)
    }
}

fn repair_directory(
    file: &mut File,
    volume: &Volume,
    cluster: u32,
    dot_dot: u32,
    depth: usize,
    seen: &mut HashSet<u32>,
) -> Result<(), Error> {
    if depth > MAX_DEPTH || !seen.insert(cluster) {
        return Ok(());
    }
    let clusters = volume.chain(file, cluster)?;
    if clusters.is_empty() || clusters.len() > MAX_DIRECTORY_CLUSTERS {
        return Ok(());
    }
    let mut bytes = read_clusters(file, volume, &clusters)?;

    let entries = fixed_entries(&bytes, cluster, dot_dot);
    let used = entries.len() * ENTRY;
    let changed = bytes[..used.min(bytes.len())] != entries.concat()[..];
    let children = subdirectories(&entries);
    if changed {
        bytes.fill(0);
        for (index, entry) in entries.iter().enumerate() {
            bytes[index * ENTRY..(index + 1) * ENTRY].copy_from_slice(entry);
        }
        write_clusters(file, volume, &clusters, &bytes)?;
    }
    // A child's `..` is 0 when this directory is the root, and this directory's cluster otherwise.
    let parent = if cluster == volume.root { 0 } else { cluster };
    for child in children {
        repair_directory(file, volume, child, parent, depth + 1, seen)?;
    }
    Ok(())
}

/// The directory's entries up to its end marker, without the long-name entries that sit in front of
/// `.` and `..`, and with `.` and `..` pointing where they should.
fn fixed_entries(bytes: &[u8], cluster: u32, dot_dot: u32) -> Vec<[u8; ENTRY]> {
    let mut entries: Vec<[u8; ENTRY]> = Vec::new();
    let mut long_names: Vec<[u8; ENTRY]> = Vec::new();
    for chunk in bytes.chunks_exact(ENTRY) {
        let mut entry = [0_u8; ENTRY];
        entry.copy_from_slice(chunk);
        if entry[0] == 0 {
            break;
        }
        if entry[11] == ATTR_LONG_NAME {
            long_names.push(entry);
            continue;
        }
        let name = &entry[..11];
        if name == DOT || name == DOT_DOT {
            // Drop the long name that came with it; the dot entries have none.
            long_names.clear();
            let target = if name == DOT { cluster } else { dot_dot };
            set_first_cluster(&mut entry, target);
        } else {
            entries.append(&mut long_names);
        }
        entries.push(entry);
    }
    entries.append(&mut long_names);
    entries
}

fn subdirectories(entries: &[[u8; ENTRY]]) -> Vec<u32> {
    entries
        .iter()
        .filter(|e| {
            e[11] != ATTR_LONG_NAME
                && e[11] & ATTR_DIRECTORY != 0
                && e[0] != DELETED
                && &e[..11] != DOT
                && &e[..11] != DOT_DOT
        })
        .map(first_cluster)
        .filter(|cluster| *cluster >= 2)
        .collect()
}

fn first_cluster(entry: &[u8; ENTRY]) -> u32 {
    let high = u32::from(u16::from_le_bytes([entry[20], entry[21]]));
    let low = u32::from(u16::from_le_bytes([entry[26], entry[27]]));
    (high << 16) | low
}

fn set_first_cluster(entry: &mut [u8; ENTRY], cluster: u32) {
    let high = u16::try_from(cluster >> 16).unwrap_or(0);
    let low = u16::try_from(cluster & 0xFFFF).unwrap_or(0);
    entry[20..22].copy_from_slice(&high.to_le_bytes());
    entry[26..28].copy_from_slice(&low.to_le_bytes());
}

fn read_clusters(file: &mut File, volume: &Volume, clusters: &[u32]) -> Result<Vec<u8>, Error> {
    let size = usize::try_from(volume.cluster_bytes).unwrap_or(usize::MAX);
    let mut bytes = vec![0_u8; size * clusters.len()];
    for (index, cluster) in clusters.iter().enumerate() {
        seek(file, volume.cluster_offset(*cluster))?;
        file.read_exact(&mut bytes[index * size..(index + 1) * size])
            .map_err(|e| Error::io("reading a directory", e))?;
    }
    Ok(bytes)
}

fn write_clusters(
    file: &mut File,
    volume: &Volume,
    clusters: &[u32],
    bytes: &[u8],
) -> Result<(), Error> {
    let size = usize::try_from(volume.cluster_bytes).unwrap_or(usize::MAX);
    for (index, cluster) in clusters.iter().enumerate() {
        seek(file, volume.cluster_offset(*cluster))?;
        file.write_all(&bytes[index * size..(index + 1) * size])
            .map_err(|e| Error::io("repairing a directory", e))?;
    }
    file.sync_all()
        .map_err(|e| Error::io("flushing the repaired directory", e))
}

fn seek(file: &mut File, position: u64) -> Result<(), Error> {
    file.seek(SeekFrom::Start(position))
        .map(|_| ())
        .map_err(|e| Error::io("moving inside the EFI partition", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &[u8; 11], attr: u8, cluster: u32) -> [u8; ENTRY] {
        let mut e = [0_u8; ENTRY];
        e[..11].copy_from_slice(name);
        e[11] = attr;
        set_first_cluster(&mut e, cluster);
        e
    }

    fn long_name() -> [u8; ENTRY] {
        let mut e = [0_u8; ENTRY];
        e[0] = 0x41;
        e[11] = ATTR_LONG_NAME;
        e
    }

    fn bytes_of(entries: &[[u8; ENTRY]]) -> Vec<u8> {
        let mut bytes = entries.concat();
        bytes.resize(512, 0);
        bytes
    }

    #[test]
    fn removes_the_long_names_in_front_of_the_dot_entries_and_keeps_the_rest() {
        let file_long = long_name();
        let file = entry(b"CONFIG~1JSO", 0x20, 9);
        let bytes = bytes_of(&[
            long_name(),
            entry(DOT, ATTR_DIRECTORY, 7),
            long_name(),
            entry(DOT_DOT, ATTR_DIRECTORY, 2),
            file_long,
            file,
        ]);
        let fixed = fixed_entries(&bytes, 7, 0);
        assert_eq!(fixed.len(), 4);
        assert_eq!(&fixed[0][..11], DOT);
        assert_eq!(first_cluster(&fixed[0]), 7);
        assert_eq!(&fixed[1][..11], DOT_DOT);
        assert_eq!(first_cluster(&fixed[1]), 0, "the parent is the root");
        assert_eq!(fixed[2], file_long);
        assert_eq!(fixed[3], file);
    }

    #[test]
    fn leaves_a_correct_directory_as_it_is() {
        let entries = [
            entry(DOT, ATTR_DIRECTORY, 7),
            entry(DOT_DOT, ATTR_DIRECTORY, 3),
            long_name(),
            entry(b"NOTES   TXT", 0x20, 9),
        ];
        assert_eq!(fixed_entries(&bytes_of(&entries), 7, 3), entries);
    }
}
