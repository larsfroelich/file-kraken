use crate::processing::find_duplicates::{
    find_file_duplicates, FindDuplicatesState, FindDuplicatesStateType,
};
use crate::utils::ui_elements::colored_box;
use crate::{state, FileKrakenApp};
use egui::Ui;
use std::ops::Deref;
use std::thread;

impl FileKrakenApp {
    pub fn files_tab_duplicates(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            colored_box(ui, egui::Color32::LIGHT_GRAY, |ui| {
                ui.label("File Duplicates");
            });
            colored_box(ui, egui::Color32::TRANSPARENT, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Status: ");
                    match self
                        .app_state
                        .find_duplicates_processing
                        .state
                        .read()
                        .unwrap()
                        .deref()
                    {
                        FindDuplicatesStateType::None => {
                            ui.label("Idle");
                        }
                        FindDuplicatesStateType::Processing(message) => {
                            ui.label("Processing");
                            ui.spinner();
                            ui.label(": ");
                            ui.label(message);
                        }
                        FindDuplicatesStateType::Processed => {
                            ui.label("Finished");
                        }
                    }
                });
            });
            if ui.button("Find Duplicates").clicked() {
                // new thread
                let _app_state = self.app_state.clone();
                thread::spawn(move || {
                    find_file_duplicates(_app_state.clone());
                });
            }
        });
    }
}
