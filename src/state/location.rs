use std::fmt;
use std::fmt::Formatter;

#[derive(PartialEq, Default, Debug, Clone)]
pub struct FileKrakenLocation {
    pub path: String,
    pub location_type: FileKrakenLocationType,
    pub parent_location_path: Option<String>
}

#[derive(PartialEq, Default, Debug, Clone)]
pub enum FileKrakenLocationType {
    #[default]
    Normal,
    Preferred,
    Excluded
}
impl fmt::Display for FileKrakenLocationType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", match self {
            FileKrakenLocationType::Normal => "normal",
            FileKrakenLocationType::Preferred => "preferred",
            FileKrakenLocationType::Excluded => "excluded"
        })
    }
}