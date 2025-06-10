pub mod tab_files;
pub mod tab_locations;

#[derive(Default, PartialEq)]
pub enum FileKrakenMainTabs {
    #[default]
    Locations,
    Files,
}
