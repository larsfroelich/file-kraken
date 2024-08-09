use sha2::{Digest, Sha256};
use std::{fs, io};

pub fn hash_file(file_path: &str) -> String {
    let mut hasher = Sha256::new();
    let mut file =
        fs::File::open(file_path).expect(&format!("Failed to open file {:?}", file_path));
    let _bytes_written =
        io::copy(&mut file, &mut hasher).expect(&format!("Failed to hash file {:?}", file_path));

    format!("{:X}", hasher.finalize())
}
