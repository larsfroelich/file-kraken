mod tab_locations;
mod tab_files;

#[derive(Default, PartialEq)]
pub enum FileKrakenMainTabs {
    #[default]
    Locations,
    Files
}