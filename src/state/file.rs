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
