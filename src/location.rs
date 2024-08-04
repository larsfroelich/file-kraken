pub struct FileKrakenLocation {
    pub path: String,
    pub location_type: FileKrakenLocationType
}

#[derive(PartialEq, Default, Debug)]
pub enum FileKrakenLocationType {
    #[default]
    Normal,
    Preferred,
    Excluded
}