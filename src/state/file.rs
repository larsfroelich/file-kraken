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
}

#[derive(PartialEq, Default, Debug, Clone)]
pub enum FileKrakenFileType {
    #[default]
    Normal,
    Archive,
}

impl FileKrakenFileType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileKrakenFileType::Normal => "normal",
            FileKrakenFileType::Archive => "archive",
        }
    }
}

impl fmt::Display for FileKrakenFileType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
