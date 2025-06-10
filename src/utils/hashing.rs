use sha2::{Digest, Sha256};
use std::{fs, io};

/// Calculate the SHA256 hash of a file.
///
/// Instead of panicking on IO errors, this function now
/// returns a `Result` so callers can react appropriately.
pub fn hash_file(file_path: &str) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut file = fs::File::open(file_path)?;
    io::copy(&mut file, &mut hasher)?;

    Ok(format!("{:X}", hasher.finalize()))
}
