use egui::vec2;
use crate::FileKrakenApp;

impl FileKrakenApp {
    pub fn locations_tab(&mut self, ui: &mut egui::Ui) {

        ui.horizontal_centered(|ui| {
            let width = ui.available_size_before_wrap().x;
            let height = ui.available_size_before_wrap().y;
            ui.columns(
                2,
                |cols| {
                    cols[0].label("Locations");
                    cols[1].label("Locations");
                }
            );
            /*
            ui.allocate_ui_with_layout(vec2(width/2.0, height), egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                ui.li
            });
            ui.allocate_ui_with_layout(vec2(width/2.0, height), egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                ui.label("Locations");
            });*/
        });
    }
}