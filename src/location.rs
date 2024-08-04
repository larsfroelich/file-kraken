struct FileKrakenLocation {
    pub path: String,
    pub location_type: FileKrakenLocationType
}

#[derive(PartialEq)]
pub enum FileKrakenLocationType {
    Normal,
    Preferred,
    Excluded
}