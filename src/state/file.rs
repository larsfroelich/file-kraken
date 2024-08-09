

pub struct FileKrakenFile {
    pub path: String,
    pub file_type: FileKrakenFileType
}

#[derive(PartialEq, Default, Debug)]
pub enum FileKrakenFileType {
    #[default]
    Normal,
    Archive,
}