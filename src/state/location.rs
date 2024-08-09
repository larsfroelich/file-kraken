use std::fmt;
use std::fmt::Formatter;

#[derive(PartialEq, Default, Debug, Clone)]
pub struct FileKrakenLocation {
    pub path: String,
    pub location_type: FileKrakenLocationType,
    pub location_state: FileKrakenLocationState,
    pub parent_location_path: Option<String>,
}

#[derive(PartialEq, Default, Debug, Clone)]
pub enum FileKrakenLocationState {
    #[default]
    Unscanned,
    PartialScanned,
    Scanned,
    Scanning,
}

impl fmt::Display for FileKrakenLocationState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                FileKrakenLocationState::Unscanned => "unscanned",
                FileKrakenLocationState::PartialScanned => "partial_scanned",
                FileKrakenLocationState::Scanned => "scanned",
                FileKrakenLocationState::Scanning => "scanning",
            }
        )
    }
}

#[derive(PartialEq, Default, Debug, Clone)]
pub enum FileKrakenLocationType {
    #[default]
    Normal,
    Preferred,
    Excluded,
}
impl fmt::Display for FileKrakenLocationType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                FileKrakenLocationType::Normal => "normal",
                FileKrakenLocationType::Preferred => "preferred",
                FileKrakenLocationType::Excluded => "excluded",
            }
        )
    }
}
