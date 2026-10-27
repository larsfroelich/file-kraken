use std::fs;
use std::fs::File;
use std::io;
use std::path::Path;
use zip::{ZipArchive, ZipWriter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveSnapshot {
    pub path: String,
    pub size: u64,
    pub mtime: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberIdentity {
    pub index: usize,
    pub name: String,
}

#[derive(Debug)]
pub enum RewriteError {
    IoError(io::Error),
    ZipError(zip::result::ZipError),
    StaleSnapshot,
    MemberMismatch(usize, String),
    InvalidPath,
}

impl From<io::Error> for RewriteError {
    fn from(err: io::Error) -> Self {
        RewriteError::IoError(err)
    }
}
impl From<zip::result::ZipError> for RewriteError {
    fn from(err: zip::result::ZipError) -> Self {
        RewriteError::ZipError(err)
    }
}

pub fn rewrite_archive(
    snapshot: &ArchiveSnapshot,
    members_to_remove: &[MemberIdentity],
) -> Result<(), RewriteError> {
    let archive_path = Path::new(&snapshot.path);

    let metadata = fs::metadata(archive_path).map_err(RewriteError::IoError)?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .unwrap_or_default()
        .as_secs();

    if metadata.len() != snapshot.size || mtime != snapshot.mtime {
        return Err(RewriteError::StaleSnapshot);
    }

    let archive_file = File::open(archive_path).map_err(RewriteError::IoError)?;
    let mut zip_archive = ZipArchive::new(archive_file).map_err(RewriteError::ZipError)?;

    let mut remove_indices = std::collections::HashSet::new();
    for member in members_to_remove {
        let zip_member = zip_archive
            .by_index(member.index)
            .map_err(RewriteError::ZipError)?;

        let raw_name = zip_member
            .name_raw()
            .iter()
            .map(|b| *b as char)
            .collect::<String>();
        if raw_name != member.name {
            return Err(RewriteError::MemberMismatch(
                member.index,
                member.name.clone(),
            ));
        }
        remove_indices.insert(member.index);
    }

    let parent_dir = archive_path.parent().ok_or(RewriteError::InvalidPath)?;
    let file_name = archive_path.file_name().ok_or(RewriteError::InvalidPath)?;

    let temp_name = format!(".tmp_rewrite_{}", file_name.to_string_lossy());
    let temp_path = parent_dir.join(temp_name);

    {
        let temp_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(RewriteError::IoError)?;
        let mut zip_writer = ZipWriter::new(temp_file);

        for i in 0..zip_archive.len() {
            if remove_indices.contains(&i) {
                continue;
            }
            let raw_file = zip_archive
                .by_index_raw(i)
                .map_err(RewriteError::ZipError)?;
            zip_writer
                .raw_copy_file(raw_file)
                .map_err(RewriteError::ZipError)?;
        }

        zip_writer.finish().map_err(RewriteError::ZipError)?;
    }

    fs::rename(&temp_path, archive_path).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        RewriteError::IoError(e)
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::FileOptions;
    use zip::CompressionMethod;

    #[test]
    fn test_rewrite_archive_basic() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("test.zip");

        {
            let file = File::create(&archive_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options: FileOptions<()> =
                FileOptions::default().compression_method(CompressionMethod::Stored);

            zip.start_file("file1.txt", options.clone()).unwrap();
            zip.write_all(b"content1").unwrap();

            zip.start_file("file2.txt", options.clone()).unwrap();
            zip.write_all(b"content2").unwrap();

            zip.start_file("file3.txt", options).unwrap();
            zip.write_all(b"content3").unwrap();

            zip.finish().unwrap();
        }

        let metadata = fs::metadata(&archive_path).unwrap();
        let mtime = metadata
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let size = metadata.len();

        let snapshot = ArchiveSnapshot {
            path: archive_path.to_str().unwrap().to_string(),
            size,
            mtime,
        };

        let remove = vec![MemberIdentity {
            index: 1,
            name: "file2.txt".to_string(),
        }];
        let result = rewrite_archive(&snapshot, &remove);
        assert!(result.is_ok(), "Rewrite failed: {:?}", result);

        let file = File::open(&archive_path).unwrap();
        let mut zip = ZipArchive::new(file).unwrap();

        assert_eq!(zip.len(), 2);

        {
            let m1 = zip.by_index(0).unwrap();
            assert_eq!(
                String::from_utf8_lossy(m1.name_raw()).into_owned(),
                "file1.txt"
            );
        }
        {
            let m2 = zip.by_index(1).unwrap();
            assert_eq!(
                String::from_utf8_lossy(m2.name_raw()).into_owned(),
                "file3.txt"
            );
        }
    }

    #[test]
    fn test_stale_snapshot() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("test.zip");

        {
            let file = File::create(&archive_path).unwrap();
            let mut zip = ZipWriter::new(file);
            zip.start_file("f1", FileOptions::default() as FileOptions<()>)
                .unwrap();
            zip.finish().unwrap();
        }

        let snapshot = ArchiveSnapshot {
            path: archive_path.to_str().unwrap().to_string(),
            size: 0,
            mtime: 0,
        };

        let result = rewrite_archive(&snapshot, &[]);
        assert!(matches!(result, Err(RewriteError::StaleSnapshot)));
    }
}
