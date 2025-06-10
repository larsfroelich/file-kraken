use crate::state::file::FileKrakenFile;
use std::fmt;

#[derive(Default, Debug, Clone)]
pub struct FileKrakenDuplicate {
    /// The file that will be deleted in favor of the `other_files`
    pub deletable_file: Option<FileKrakenFile>,
    pub other_files: Vec<FileKrakenFile>,
    pub duplicate_type: FileKrakenDuplicateType,
}

#[derive(PartialEq, Default, Debug, Clone)]
pub enum FileKrakenDuplicateType {
    #[default]
    ExactMatch,
}

impl fmt::Display for FileKrakenDuplicateType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileKrakenDuplicateType::ExactMatch => write!(f, "Exact"),
        }
    }
}
