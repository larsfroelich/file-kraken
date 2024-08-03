#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![allow(rustdoc::missing_crate_level_docs)] // it's an example

mod tabs;
mod app_init;

use crate::app_init::app_init;
use crate::tabs::FileKrakenMainTabs;

#[derive(Default)]
pub struct FileKrakenApp {
    name: String,
    age: u32,
    tabs: FileKrakenMainTabs,
}


impl eframe::App for FileKrakenApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("File Kraken");
            ui.horizontal(|ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tabs, FileKrakenMainTabs::Locations, "Locations");
                    ui.selectable_value(&mut self.tabs, FileKrakenMainTabs::Files, "Files");
                });
            });
            match self.tabs {
                FileKrakenMainTabs::Locations => self.locations_tab(ui),
                FileKrakenMainTabs::Files => self.files_tab(ui),
            }
            /*
            ui.horizontal(|ui| {
                let name_label = ui.label("Your name: ");
                ui.text_edit_singleline(&mut self.name)
                    .labelled_by(name_label.id);
            });
            ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
            if ui.button("Increment").clicked() {
                self.age += 1;
            }
            ui.label(format!("Hello '{}', age {}", self.name, self.age));
*/
        });
    }
}

fn main() -> eframe::Result {
    // init env logger
    env_logger::init();
    
    // run the app
    app_init()
}


