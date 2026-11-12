use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::Error;

const CHUNK: usize = 4 * 1024 * 1024;

/// Copies an image and returns the SHA-256 of what was written, as lowercase hex.
///
/// The source is only read. The destination must not exist, so a copy can never overwrite an
/// image. `progress` is called with the bytes copied so far and the total. When the copy fails,
/// the partial destination is removed, so it cannot be mistaken for a good image.
pub fn copy_image(src: &Path, dst: &Path, progress: impl FnMut(u64, u64)) -> Result<String, Error> {
    let mut source =
        File::open(src).map_err(|e| Error::io(format!("opening {}", src.display()), e))?;
    let total = source
        .metadata()
        .map_err(|e| Error::io("reading the image size", e))?
        .len();

    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dst)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Error::DestinationExists(dst.to_path_buf())
            } else {
                Error::io(format!("creating {}", dst.display()), e)
            }
        })?;

    let result = copy_and_hash(&mut source, &mut target, total, progress);
    if result.is_err() {
        drop(target);
        let _ = fs::remove_file(dst);
    }
    result
}

fn copy_and_hash(
    source: &mut File,
    target: &mut File,
    total: u64,
    mut progress: impl FnMut(u64, u64),
) -> Result<String, Error> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; CHUNK];
    let mut copied = 0_u64;
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|e| Error::io("reading the image", e))?;
        if read == 0 {
            break;
        }
        target
            .write_all(&buffer[..read])
            .map_err(|e| Error::io("writing the copy", e))?;
        hasher.update(&buffer[..read]);
        copied += read as u64;
        progress(copied, total);
    }
    target
        .sync_all()
        .map_err(|e| Error::io("flushing the copy", e))?;
    Ok(hex::encode(hasher.finalize()))
}

/// Whether the file at `path` has the SHA-256 `expected` (lowercase hex).
pub fn verify_image(path: &Path, expected: &str) -> Result<bool, Error> {
    let mut file =
        File::open(path).map_err(|e| Error::io(format!("opening {}", path.display()), e))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; CHUNK];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| Error::io("reading the image", e))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()).eq_ignore_ascii_case(expected))
}
