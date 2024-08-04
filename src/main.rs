#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![allow(rustdoc::missing_crate_level_docs)] // it's an example

mod tabs;
mod app_init;
mod location;

use egui::{Align, FontId, Layout, RichText, vec2, Vec2};
use crate::app_init::app_init;
use crate::tabs::FileKrakenMainTabs;

#[derive(Default)]
pub struct FileKrakenApp {
    tabs: FileKrakenMainTabs,
    selected_location: Option<u32>,
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

impl eframe::App for FileKrakenApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.allocate_ui_with_layout(Vec2::new(0.0,0.0), Layout::left_to_right(Align::BOTTOM) ,|ui| {
                ui.label(RichText::new("File Kraken").font(FontId::proportional(40.0)).line_height(Some(40.0)));
                ui.label(RichText::new(String::from(" v") + VERSION).font(FontId::proportional(20.0)).line_height(Some(40.0)));
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tabs, FileKrakenMainTabs::Locations, RichText::new("Locations"));
                    ui.selectable_value(&mut self.tabs, FileKrakenMainTabs::Files, RichText::new("Files"));
                });
            });
            ui.separator();
            match self.tabs {
                FileKrakenMainTabs::Locations => self.locations_tab(ui, None),
                FileKrakenMainTabs::Files => self.files_tab(ui),
            }
        });
    }
}

fn main() -> eframe::Result {
    // init env logger
    env_logger::init();

    // run the app
    app_init()
}


