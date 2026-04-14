use crate::utils::ui_elements::colored_box;
use crate::FileKrakenApp;
use egui::Ui;

impl FileKrakenApp {
    pub fn files_tab_overview(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            colored_box(
                ui,
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
                |ui| {
                    ui.label("Files Overview");
                },
            );
            colored_box(ui, egui::Color32::TRANSPARENT, egui::Stroke::NONE, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Total Files: ");
                    ui.label(self.app_state.get_total_files_count().to_string());
                });
            });
        });
    }
}
