use crate::state::file::FileKrakenFile;

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
