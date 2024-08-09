use crate::state::file::FileKrakenFile;

pub struct FileKrakenDuplicate {
    pub files: Vec<FileKrakenFile>,
    pub primary_file: FileKrakenFile,
    pub duplicate_type: FileKrakenDuplicateType,
}

pub enum FileKrakenDuplicateType {
    ExactMatch,
}
