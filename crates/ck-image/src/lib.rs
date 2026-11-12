//! Reads and changes disk image files without Linux tools, so the admin tool can run on Windows.
//!
//! Images are about 30 GB and are never loaded into memory: the partition table is read from the
//! file, and the EFI partition is edited in place as a FAT volume.

mod copy;
mod efi;
mod error;
mod gpt;

pub use copy::{copy_image, verify_image};
pub use efi::{read_file, write_file};
pub use error::Error;
pub use gpt::{Guid, Partition, SECTOR_SIZE, find_efi, find_linux_root, read_partitions};
