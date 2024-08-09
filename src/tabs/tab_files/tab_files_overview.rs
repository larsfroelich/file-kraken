use crate::utils::ui_elements::colored_box;
use crate::FileKrakenApp;
use egui::Ui;

impl FileKrakenApp {
    pub fn files_tab_overview(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            colored_box(ui, egui::Color32::LIGHT_GRAY, |ui| {
                ui.label("Files Overview");
            });
            colored_box(ui, egui::Color32::TRANSPARENT, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Total Files: ");
                    let mut total_files = 0;
                    for location in self.app_state.get_locations_list_readonly().iter() {
                        total_files += self
                            .app_state
                            .get_files_by_location(&location.path)
                            .unwrap()
                            .read()
                            .unwrap()
                            .len()
                    }
                    ui.label(total_files.to_string());
                });
            });
        });
    }
}
