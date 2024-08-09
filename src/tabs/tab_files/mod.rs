use crate::FileKrakenApp;
use egui::RichText;

mod tab_files_duplicates;
mod tab_files_overview;

#[derive(Default, PartialEq)]
pub enum FileKrakenFileTabs {
    #[default]
    Overview,
    Duplicates,
}

impl FileKrakenApp {
    pub fn files_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.current_files_tab,
                    FileKrakenFileTabs::Overview,
                    RichText::new("Overview"),
                );
                ui.selectable_value(
                    &mut self.current_files_tab,
                    FileKrakenFileTabs::Duplicates,
                    RichText::new("Duplicates"),
                );
            });
        });
        ui.separator();
        match self.current_files_tab {
            FileKrakenFileTabs::Overview => self.files_tab_overview(ui),
            FileKrakenFileTabs::Duplicates => self.files_tab_duplicates(ui),
        }
    }
}
