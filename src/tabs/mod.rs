
pub mod tab_locations;
pub mod tab_files;

#[derive(Default, PartialEq)]
pub enum FileKrakenMainTabs {
    #[default]
    Locations,
    Files
}