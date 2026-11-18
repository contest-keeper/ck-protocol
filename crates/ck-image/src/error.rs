use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
    #[error("this file has no partition table (the 'EFI PART' signature is missing)")]
    NoPartitionTable,
    #[error("the partition table is damaged: {0}")]
    BadPartitionTable(&'static str),
    #[error("the image has no EFI partition")]
    NoEfiPartition,
    #[error("the image has no Linux root partition")]
    NoLinuxPartition,
    #[error("the EFI partition cannot be read as a FAT volume: {0}")]
    Fat(String),
    #[error("'{0}' is not a valid path inside the EFI partition")]
    BadPath(String),
    #[error("'{0}' not found in the EFI partition")]
    FileNotFound(String),
    #[error("{} already exists; a copy never replaces an existing file", .0.display())]
    DestinationExists(PathBuf),
}

impl Error {
    pub(crate) fn io(context: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}
