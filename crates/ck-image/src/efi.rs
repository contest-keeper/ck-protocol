use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use fatfs::{FileSystem, FsOptions};
use fscommon::{BufStream, StreamSlice};

use crate::error::Error;
use crate::gpt::{find_efi, read_partitions};

type EfiVolume = FileSystem<BufStream<StreamSlice<File>>>;

/// Writes a file inside the image's EFI partition, in place, creating folders as needed and
/// replacing any existing content. The rest of the image is never touched.
///
/// `path` uses `/`, for example `ck/config.json`.
pub fn write_file(image: &Path, path: &str, content: &[u8]) -> Result<(), Error> {
    let (folders, name) = split_path(path)?;
    let volume = open_efi(image, true)?;
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
    volume.unmount().map_err(fat)
}

/// Reads a file from the image's EFI partition.
pub fn read_file(image: &Path, path: &str) -> Result<Vec<u8>, Error> {
    let (folders, name) = split_path(path)?;
    let volume = open_efi(image, false)?;
    let mut folder = volume.root_dir();
    for part in folders {
        folder = folder
            .open_dir(part)
            .map_err(|_| Error::FileNotFound(path.to_owned()))?;
    }
    let mut file = folder
        .open_file(name)
        .map_err(|_| Error::FileNotFound(path.to_owned()))?;
    let mut content = Vec::new();
    file.read_to_end(&mut content)
        .map_err(|source| Error::io(format!("reading {path}"), source))?;
    Ok(content)
}

fn open_efi(image: &Path, writable: bool) -> Result<EfiVolume, Error> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(writable)
        .open(image)
        .map_err(|source| Error::io(format!("opening {}", image.display()), source))?;
    let partitions = read_partitions(&mut file)?;
    let efi = find_efi(&partitions)?;
    let slice = StreamSlice::new(file, efi.start_byte(), efi.end_byte())
        .map_err(|source| Error::io("selecting the EFI partition", source))?;
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

fn fat(error: impl std::fmt::Debug) -> Error {
    Error::Fat(format!("{error:?}"))
}
