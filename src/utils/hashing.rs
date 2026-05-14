use sha2::{Digest, Sha256};
use std::{fs, io};

/// Calculate the SHA256 hash of a file.
///
/// Instead of panicking on IO errors, this function now
/// returns a `Result` so callers can react appropriately.
pub fn hash_file(file_path: &str) -> io::Result<String> {
    use std::io::Read;

    let mut hasher = Sha256::new();
    let mut file = fs::File::open(file_path)?;
    let mut buffer = vec![0; 8 * 1024 * 1024]; // 8MB buffer

    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(format!("{:X}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::hash_file;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_hash_file_success() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("sample.txt");
        let mut file = File::create(&file_path).unwrap();
        write!(file, "hello world").unwrap();

        let hash = hash_file(file_path.to_str().unwrap()).unwrap();
        assert_eq!(
            hash,
            "B94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9"
        );
    }

    #[test]
    fn test_hash_file_not_found() {
        let err = hash_file("/non/existent/path.txt").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }
}
