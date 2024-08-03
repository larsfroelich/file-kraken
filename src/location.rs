struct FileKrakenLocation {
    pub path: String,
    pub location_type: FileKrakenLocationType
}

pub enum FileKrakenLocationType {
    Normal,
    Preferred,
    Excluded
}