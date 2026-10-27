use std::fmt;
use std::fmt::Formatter;

#[derive(Default, Debug, Clone)]
pub struct FileKrakenFile {
    pub path: String,
    pub file_type: FileKrakenFileType,
    pub file_len: u64,
    pub time_created: u64,
    pub time_modified: u64,
    pub hash: Option<String>,
    pub archive_path: Option<String>,
    pub archive_size: Option<u64>,
    pub archive_mtime: Option<u64>,
}

impl FileKrakenFile {
    pub fn display_path(&self) -> String {
        if let Some(archive_path) = &self.archive_path {
            if let Some(rest) = self.path.strip_prefix(&format!("{}::", archive_path)) {
                let parts: Vec<&str> = rest.splitn(2, "::").collect();
                if parts.len() == 2 {
                    return format!("{}::{}", archive_path, parts[1]);
                }
            }
            self.path.clone()
        } else {
            self.path.clone()
        }
    }
}

#[derive(PartialEq, Default, Debug, Clone)]
pub enum FileKrakenFileType {
    #[default]
    Normal,
    Archive,
    ArchiveMember,
}

impl FileKrakenFileType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileKrakenFileType::Normal => "normal",
            FileKrakenFileType::Archive => "archive",
            FileKrakenFileType::ArchiveMember => "archive_member",
        }
    }
}

impl fmt::Display for FileKrakenFileType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
