use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use fatfs::{FileSystem, FsOptions};
use fscommon::{BufStream, StreamSlice};

use crate::dot_entries;
use crate::error::Error;
use crate::gpt::{Partition, find_config, find_efi, read_partitions};

type EfiVolume = FileSystem<BufStream<StreamSlice<File>>>;

/// Which partition a read or write goes to.
#[derive(Clone, Copy)]
enum Target {
    Efi,
    Config,
}

impl Target {
    fn find(self, partitions: &[Partition]) -> Result<&Partition, Error> {
        match self {
            Self::Efi => find_efi(partitions),
            Self::Config => find_config(partitions).ok_or(Error::NoConfigPartition),
        }
    }
}

/// Whether the image has a `CKCONFIG` partition.
pub fn has_config_partition(image: &Path) -> Result<bool, Error> {
    let mut file = File::open(image)
        .map_err(|source| Error::io(format!("opening {}", image.display()), source))?;
    Ok(find_config(&read_partitions(&mut file)?).is_some())
}

/// Writes a file at the root of the image's `CKCONFIG` partition, in place, replacing any existing
/// file of that name. `name` is a plain file name. Nothing else in the image is touched.
pub fn write_config_file(image: &Path, name: &str, content: &[u8]) -> Result<(), Error> {
    check_plain_name(name)?;
    let volume = open_volume(image, true, Target::Config)?;
    {
        let mut file = volume.root_dir().create_file(name).map_err(fat)?;
        file.truncate().map_err(fat)?;
        file.write_all(content).map_err(fat)?;
        file.flush().map_err(fat)?;
    }
    volume.unmount().map_err(fat)
}

/// Reads a file from the root of the image's `CKCONFIG` partition.
pub fn read_config_file(image: &Path, name: &str) -> Result<Vec<u8>, Error> {
    check_plain_name(name)?;
    let volume = open_volume(image, false, Target::Config)?;
    let mut file = volume
        .root_dir()
        .open_file(name)
        .map_err(|e| missing_or_fat(e, name))?;
    let mut content = Vec::new();
    file.read_to_end(&mut content)
        .map_err(|source| Error::io(format!("reading {name}"), source))?;
    Ok(content)
}

fn check_plain_name(name: &str) -> Result<(), Error> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
        return Err(Error::BadPath(name.to_owned()));
    }
    Ok(())
}

/// Writes a file inside the image's EFI partition, in place, creating folders as needed and
/// replacing any existing content. The rest of the image is never touched.
///
/// `path` uses `/`, for example `ck/config.json`.
pub fn write_file(image: &Path, path: &str, content: &[u8]) -> Result<(), Error> {
    let (folders, name) = split_path(path)?;
    let volume = open_volume(image, true, Target::Efi)?;
    {
        let mut folder = volume.root_dir();
        for part in folders {
            folder = folder.create_dir(part).map_err(fat)?;
        }
        let mut file = folder.create_file(name).map_err(fat)?;
        file.truncate().map_err(fat)?;
        file.write_all(content).map_err(fat)?;
        file.flush().map_err(fat)?;
    }
    volume.unmount().map_err(fat)?;
    repair_dot_entries(image)
}

/// `fatfs` writes the `.` and `..` of a new folder in a way Linux `fsck` calls damage and repairs at
/// every boot, so they are put right after each write. See the `dot_entries` module.
fn repair_dot_entries(image: &Path) -> Result<(), Error> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(image)
        .map_err(|source| Error::io(format!("opening {}", image.display()), source))?;
    let partitions = read_partitions(&mut file)?;
    let efi = find_efi(&partitions)?;
    dot_entries::repair(&mut file, efi.start_byte())
}

pub fn read_file(image: &Path, path: &str) -> Result<Vec<u8>, Error> {
    let (folders, name) = split_path(path)?;
    let volume = open_volume(image, false, Target::Efi)?;
    let mut folder = volume.root_dir();
    for part in folders {
        folder = folder.open_dir(part).map_err(|e| missing_or_fat(e, path))?;
    }
    let mut file = folder
        .open_file(name)
        .map_err(|e| missing_or_fat(e, path))?;
    let mut content = Vec::new();
    file.read_to_end(&mut content)
        .map_err(|source| Error::io(format!("reading {path}"), source))?;
    Ok(content)
}

fn open_volume(image: &Path, writable: bool, target: Target) -> Result<EfiVolume, Error> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(writable)
        .open(image)
        .map_err(|source| Error::io(format!("opening {}", image.display()), source))?;
    let partitions = read_partitions(&mut file)?;
    let efi = target.find(&partitions)?;
    let file_length = file
        .metadata()
        .map_err(|source| Error::io("reading the image size", source))?
        .len();
    if efi.end_byte() > file_length {
        return Err(Error::BadPartitionTable(
            "the partition extends past the end of the file",
        ));
    }
    let slice = StreamSlice::new(file, efi.start_byte(), efi.end_byte())
        .map_err(|source| Error::io("selecting the partition", source))?;
    FileSystem::new(BufStream::new(slice), FsOptions::new()).map_err(fat)
}

fn split_path(path: &str) -> Result<(Vec<&str>, &str), Error> {
    let mut parts: Vec<&str> = path.split('/').collect();
    let valid = parts
        .iter()
        .all(|part| !part.is_empty() && *part != "." && *part != ".." && !part.contains('\\'));
    match parts.pop() {
        Some(name) if valid => Ok((parts, name)),
        _ => Err(Error::BadPath(path.to_owned())),
    }
}

/// Only a real "not found" means the file is missing; anything else is a damaged volume.
fn missing_or_fat(error: std::io::Error, path: &str) -> Error {
    if error.kind() == std::io::ErrorKind::NotFound {
        Error::FileNotFound(path.to_owned())
    } else {
        fat(error)
    }
}

fn fat(error: impl std::fmt::Debug) -> Error {
    Error::Fat(format!("{error:?}"))
}
