use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use ck_image::{
    Error, Guid, SECTOR_SIZE, copy_image, find_efi, find_linux_root, read_file, read_partitions,
    verify_image, write_file,
};
use fatfs::{FatType, FormatVolumeOptions};
use fscommon::StreamSlice;
use tempfile::TempDir;

const IMAGE_SIZE: u64 = 96 * 1024 * 1024;
const EFI_FIRST: u64 = 2048;
const EFI_LAST: u64 = EFI_FIRST + 48 * 1024 * 1024 / SECTOR_SIZE - 1;
const ROOT_FIRST: u64 = EFI_LAST + 1;
const ROOT_LAST: u64 = IMAGE_SIZE / SECTOR_SIZE - 34;
const BEFORE_EFI_BYTES: usize = 2048 * 512;
const HEADER_ENTRY_COUNT_AT: usize = 512 + 80;
const HEADER_ENTRY_SIZE_AT: usize = 512 + 84;
const HEADER_CRC_AT: usize = 512 + 16;
const FIRST_ENTRY_LAST_SECTOR_AT: usize = 2 * 512 + 40;
const HEADER_ENTRIES_CRC_AT: usize = 512 + 88;

/// Builds a small image: a GPT with an EFI partition (FAT32, label EFI) and a Linux partition filled with a
/// recognisable pattern, so a test can tell whether anything outside the EFI partition changed.
fn make_image(dir: &TempDir) -> PathBuf {
    let path = dir.path().join("test.img");
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .expect("create image");
    file.set_len(IMAGE_SIZE).expect("size image");

    write_gpt(&mut file);

    file.seek(SeekFrom::Start(ROOT_FIRST * SECTOR_SIZE))
        .expect("seek root");
    file.write_all(&[0xAB; 4096]).expect("fill root");

    let slice = StreamSlice::new(file, EFI_FIRST * SECTOR_SIZE, (EFI_LAST + 1) * SECTOR_SIZE)
        .expect("slice");
    let options = FormatVolumeOptions::new()
        .fat_type(FatType::Fat32)
        .volume_label(*b"EFI        ");
    fatfs::format_volume(slice, options).expect("format EFI");
    path
}

fn write_gpt(file: &mut File) {
    let mut entries = vec![0_u8; 128 * 128];
    for (index, (guid, unique, first, last, name)) in [
        (Guid::EFI_SYSTEM, 1_u8, EFI_FIRST, EFI_LAST, "EFI"),
        (Guid::LINUX_FILESYSTEM, 2_u8, ROOT_FIRST, ROOT_LAST, "root"),
    ]
    .into_iter()
    .enumerate()
    {
        let entry = &mut entries[index * 128..(index + 1) * 128];
        entry[..16].copy_from_slice(&guid.to_disk_bytes());
        entry[16..32].copy_from_slice(&[unique; 16]);
        entry[32..40].copy_from_slice(&first.to_le_bytes());
        entry[40..48].copy_from_slice(&last.to_le_bytes());
        for (i, unit) in name.encode_utf16().enumerate() {
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
    header[88..92].copy_from_slice(&crc32fast::hash(&entries).to_le_bytes());
    let crc = crc32fast::hash(&header);
    header[16..20].copy_from_slice(&crc.to_le_bytes());

    file.seek(SeekFrom::Start(SECTOR_SIZE))
        .expect("seek header");
    file.write_all(&header).expect("write header");
    file.seek(SeekFrom::Start(2 * SECTOR_SIZE))
        .expect("seek entries");
    file.write_all(&entries).expect("write entries");
}

fn read_bytes(path: &Path, start: u64, length: usize) -> Vec<u8> {
    let mut file = File::open(path).expect("open image");
    file.seek(SeekFrom::Start(start)).expect("seek");
    let mut bytes = vec![0; length];
    file.read_exact(&mut bytes).expect("read");
    bytes
}

#[test]
fn finds_the_efi_and_linux_partitions() {
    let dir = TempDir::new().expect("temp dir");
    let image = make_image(&dir);
    let partitions = read_partitions(&mut File::open(&image).expect("open")).expect("partitions");

    assert_eq!(partitions.len(), 2);
    let efi = find_efi(&partitions).expect("efi");
    assert_eq!(
        (efi.first_sector, efi.last_sector, efi.name.as_str()),
        (EFI_FIRST, EFI_LAST, "EFI")
    );
    assert_eq!(
        find_linux_root(&partitions).expect("root").first_sector,
        ROOT_FIRST
    );
    assert_eq!(
        Guid::EFI_SYSTEM.to_string(),
        "C12A7328-F81F-11D2-BA4B-00A0C93EC93B"
    );
    assert_eq!(
        Guid::LINUX_FILESYSTEM.to_string(),
        "0FC63DAF-8483-4772-8E79-3D69D8477DE4"
    );
}

#[test]
fn explains_a_file_that_is_not_an_image() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("notes.txt");
    fs::write(&path, vec![b'x'; 4096]).expect("write");
    let error = read_partitions(&mut File::open(&path).expect("open")).expect_err("not an image");
    assert!(matches!(error, Error::NoPartitionTable));
    assert!(error.to_string().contains("no partition table"));

    let mut damaged = fs::read(make_image(&dir)).expect("read image");
    damaged[HEADER_ENTRY_COUNT_AT] ^= 0xFF;
    let error = read_partitions(&mut std::io::Cursor::new(damaged)).expect_err("damaged");
    assert!(matches!(error, Error::BadPartitionTable(_)));
}

/// Fixes the checksums of an image whose header or entries were changed, like a crafted image would.
fn reseal(image: &mut [u8]) {
    let entries_crc = crc32fast::hash(&image[1024..1024 + 128 * 128]);
    image[HEADER_ENTRIES_CRC_AT..HEADER_ENTRIES_CRC_AT + 4]
        .copy_from_slice(&entries_crc.to_le_bytes());
    image[HEADER_CRC_AT..HEADER_CRC_AT + 4].fill(0);
    let crc = crc32fast::hash(&image[512..512 + 92]);
    image[HEADER_CRC_AT..HEADER_CRC_AT + 4].copy_from_slice(&crc.to_le_bytes());
}

#[test]
fn rejects_a_crafted_partition_table_without_crashing() {
    let dir = TempDir::new().expect("temp dir");
    let original = fs::read(make_image(&dir)).expect("read image");
    let read = |bytes: Vec<u8>| read_partitions(&mut std::io::Cursor::new(bytes));

    // A huge entry size would ask for terabytes of memory.
    let mut huge = original.clone();
    huge[HEADER_ENTRY_SIZE_AT..HEADER_ENTRY_SIZE_AT + 4]
        .copy_from_slice(&0xFFFF_0000_u32.to_le_bytes());
    reseal(&mut huge);
    assert!(matches!(read(huge), Err(Error::BadPartitionTable(_))));

    // A partition end that overflows when turned into bytes.
    let mut overflow = original.clone();
    overflow[FIRST_ENTRY_LAST_SECTOR_AT..FIRST_ENTRY_LAST_SECTOR_AT + 8]
        .copy_from_slice(&u64::MAX.to_le_bytes());
    reseal(&mut overflow);
    assert!(matches!(read(overflow), Err(Error::BadPartitionTable(_))));

    // A partition that ends before it starts.
    let mut backwards = original;
    backwards[FIRST_ENTRY_LAST_SECTOR_AT..FIRST_ENTRY_LAST_SECTOR_AT + 8]
        .copy_from_slice(&1_u64.to_le_bytes());
    reseal(&mut backwards);
    assert!(matches!(read(backwards), Err(Error::BadPartitionTable(_))));
}

#[test]
fn refuses_a_partition_that_goes_past_the_end_of_the_file() {
    let dir = TempDir::new().expect("temp dir");
    let image = make_image(&dir);
    let mut bytes = fs::read(&image).expect("read image");
    let past_the_end = IMAGE_SIZE / SECTOR_SIZE + 1000;
    bytes[FIRST_ENTRY_LAST_SECTOR_AT..FIRST_ENTRY_LAST_SECTOR_AT + 8]
        .copy_from_slice(&past_the_end.to_le_bytes());
    reseal(&mut bytes);
    fs::write(&image, bytes).expect("write image");

    assert!(matches!(
        read_file(&image, "ck/config.json"),
        Err(Error::BadPartitionTable(_))
    ));
}

#[test]
fn writes_and_reads_settings_without_touching_the_rest_of_the_image() {
    let dir = TempDir::new().expect("temp dir");
    let image = make_image(&dir);
    let root_before = read_bytes(&image, ROOT_FIRST * SECTOR_SIZE, 4096);
    let table_before = read_bytes(&image, 0, BEFORE_EFI_BYTES);

    write_file(&image, "ck/config.json", br#"{"team":"first"}"#).expect("write");
    write_file(&image, "ck/config.json", br#"{"team":"second"}"#).expect("replace");
    write_file(&image, "ck/wallpaper.png", &[7; 3000]).expect("second file");

    assert_eq!(
        read_file(&image, "ck/config.json").expect("read"),
        br#"{"team":"second"}"#
    );
    assert_eq!(
        read_file(&image, "ck/wallpaper.png").expect("read"),
        vec![7; 3000]
    );
    assert!(matches!(
        read_file(&image, "ck/missing.json"),
        Err(Error::FileNotFound(_))
    ));
    assert!(matches!(
        write_file(&image, "../escape", b"x"),
        Err(Error::BadPath(_))
    ));

    assert_eq!(
        read_bytes(&image, ROOT_FIRST * SECTOR_SIZE, 4096),
        root_before
    );
    assert_eq!(read_bytes(&image, 0, BEFORE_EFI_BYTES), table_before);
}

#[test]
fn copies_an_image_without_touching_the_source_or_an_existing_file() {
    let dir = TempDir::new().expect("temp dir");
    let image = make_image(&dir);
    let copy = dir.path().join("copy.img");
    let source_before = fs::read(&image).expect("read source");

    let mut last_progress = (0, 0);
    let hash =
        copy_image(&image, &copy, |done, total| last_progress = (done, total)).expect("copy");

    assert_eq!(last_progress, (IMAGE_SIZE, IMAGE_SIZE));
    assert_eq!(fs::read(&copy).expect("read copy"), source_before);
    assert!(verify_image(&copy, &hash).expect("verify"));

    write_file(&copy, "ck/config.json", b"{}").expect("change the copy");
    assert!(!verify_image(&copy, &hash).expect("verify"));
    assert_eq!(fs::read(&image).expect("read source"), source_before);

    assert!(matches!(
        copy_image(&image, &copy, |_, _| {}),
        Err(Error::DestinationExists(_))
    ));
    assert!(matches!(
        copy_image(&image, &image, |_, _| {}),
        Err(Error::DestinationExists(_))
    ));
}

#[cfg(unix)]
#[test]
fn a_failed_copy_leaves_no_partial_file_so_a_retry_works() {
    let dir = TempDir::new().expect("temp dir");
    let image = make_image(&dir);
    let copy = dir.path().join("copy.img");

    // Opening a folder works on Linux but reading it fails, so the copy fails after the destination
    // was created.
    assert!(matches!(
        copy_image(dir.path(), &copy, |_, _| {}),
        Err(Error::Io { .. })
    ));
    assert!(!copy.exists());
    copy_image(&image, &copy, |_, _| {}).expect("retry works");
}

/// Run by hand against a COPY of the real image: `CK_TEST_IMAGE=path/to/copy.img cargo test -p ck-image -- --ignored`.
/// It changes `ck/config.json` inside the file, so never point it at the original.
#[test]
#[ignore = "needs a copy of the real image; set CK_TEST_IMAGE"]
fn writes_settings_into_a_copy_of_the_real_image() {
    let image = PathBuf::from(
        std::env::var("CK_TEST_IMAGE").expect("set CK_TEST_IMAGE to a copy of the image"),
    );
    let partitions = read_partitions(&mut File::open(&image).expect("open")).expect("partitions");
    assert_eq!(find_efi(&partitions).expect("efi").first_sector, 4096);

    write_file(&image, "ck/config.json", br#"{"team":"manual test"}"#).expect("write");
    assert_eq!(
        read_file(&image, "ck/config.json").expect("read"),
        br#"{"team":"manual test"}"#
    );
}
