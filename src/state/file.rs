use std::sync::Arc;
use crate::state::location::FileKrakenLocation;

pub struct FileKrakenFile {
    pub location: Arc<FileKrakenLocation>,
    pub path: String,
    pub file_type: FileKrakenFileType
}

#[derive(PartialEq, Default, Debug)]
pub enum FileKrakenFileType {
    #[default]
    Normal,
    Archive,
}