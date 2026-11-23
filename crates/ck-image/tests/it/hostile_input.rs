//! Hostile input: a disk image that is not what it claims (random bytes, a GPT with wild numbers,
//! a damaged FAT volume) is an error, never a panic or a huge allocation. The number of cases
//! follows `PROPTEST_CASES`.

use std::fs;
use std::io::Cursor;

use ck_image::{has_config_partition, read_config_file, read_file, read_partitions};
use proptest::prelude::*;

/// A disk whose sector 1 carries the GPT signature and a correct header checksum around random
/// fields, so the reader gets past the first checks.
fn gpt_header_with_random_fields(fields: &[u8]) -> Vec<u8> {
    let mut disk = vec![0_u8; 512 * 40];
    let header = &mut disk[512..1024];
    header[..8].copy_from_slice(b"EFI PART");
    header[12..16].copy_from_slice(&92_u32.to_le_bytes());
    header[20..20 + fields.len()].copy_from_slice(fields);
    let crc = crc32fast::hash(&header[..92]);
    header[16..20].copy_from_slice(&crc.to_le_bytes());
    disk
}

proptest! {
    #[test]
    fn random_bytes_are_not_a_partition_table(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        let _ = read_partitions(&mut Cursor::new(bytes));
    }

    #[test]
    fn a_gpt_with_wild_header_numbers_is_refused_without_panic(
        fields in prop::collection::vec(any::<u8>(), 72),
        entries in prop::collection::vec(any::<u8>(), 0..2048),
    ) {
        let mut disk = gpt_header_with_random_fields(&fields);
        let start = 1024;
        let end = (start + entries.len()).min(disk.len());
        disk[start..end].copy_from_slice(&entries[..end - start]);
        let _ = read_partitions(&mut Cursor::new(disk));
    }

    #[test]
    fn a_file_of_random_bytes_is_no_image(
        bytes in prop::collection::vec(any::<u8>(), 0..8192),
        name in any::<String>(),
    ) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hostile.img");
        fs::write(&path, &bytes).unwrap();
        let _ = has_config_partition(&path);
        let _ = read_config_file(&path, &name);
        let _ = read_file(&path, &name);
    }
}
