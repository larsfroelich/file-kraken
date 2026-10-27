use sha2::{Digest, Sha256};
use std::{fs, io};

thread_local! {
    static BUFFER: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new(vec![0; 8 * 1024 * 1024]);
}

pub fn hash_stream<R: io::Read>(mut stream: R) -> io::Result<String> {
    let mut hasher = Sha256::new();
    BUFFER.with(|buf| {
        let mut buffer = buf.borrow_mut();
        loop {
            let count = stream.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        Ok::<(), io::Error>(())
    })?;
    Ok(format!("{:X}", hasher.finalize()))
}

pub fn hash_file(file_path: &str) -> io::Result<String> {
    let file = fs::File::open(file_path)?;
    hash_stream(file)
}

pub fn hash_archive_member(
    archive_path: &str,
    member_index: usize,
    member_name: &str,
) -> io::Result<String> {
    let file = fs::File::open(archive_path)?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut member = zip
        .by_index(member_index)
        .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))?;

    let raw_name = String::from_utf8_lossy(member.name_raw()).into_owned();
    if raw_name != member_name {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Member name mismatch",
        ));
    }

    hash_stream(&mut member)
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
