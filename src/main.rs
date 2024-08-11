#![cfg_attr(
    not(debug_assertions),
    windows_subsystem = "windows"
)] // hide console window on Windows in release
#![allow(rustdoc::missing_crate_level_docs)] // it's an example

mod app_init;
mod processing;
mod state;
mod tabs;
mod utils;

use crate::app_init::app_init;
use crate::tabs::tab_files::FileKrakenFileTabs;
use crate::tabs::tab_locations::LocationTabState;
use crate::tabs::FileKrakenMainTabs;
use crate::utils::dialogs::error_dialog;
use egui::{Align, FontId, Layout, RichText, Vec2};
use rfd::FileDialog;
use std::sync::Arc;
use std::time::Duration;

#[derive(Default)]
pub struct FileKrakenApp {
    current_tab: FileKrakenMainTabs,

    current_files_tab: FileKrakenFileTabs,

    // state for each tab
    tab_state_locations: LocationTabState,

    // main app state
    app_state: Arc<state::AppState>,
}

fn try_connect_sqlite(_self: &mut FileKrakenApp, path: &str) -> () {
    if let Err(err) = _self.app_state.connect_sqlite(path) {
        error_dialog(&format!("Failed to create project file. Error: {}", err));
    }
}

impl FileKrakenApp {
    pub fn new() -> Self {
        let mut _self = Self::default();

        if let Ok(location) = std::env::var("FILE_KRAKEN_PROJECT_FILE") {
            try_connect_sqlite(&mut _self, &location);
        }
        _self
    }
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

impl eframe::App for FileKrakenApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        std::thread::sleep(Duration::from_micros((1.0 / 120.0 * 1_000_000.0) as u64));
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.app_state.is_sqlite_connected() == false {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("File Kraken");
                        ui.add_space(10.0);
                        ui.label("Please load a project file or create a new one");
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            if ui.button("load existing project").clicked() {
                                let maybe_file = FileDialog::new()
                                    .add_filter("FileKraken Proj", &["fkrproj"])
                                    .set_directory("/")
                                    .pick_file();

                                if let Some(file) = maybe_file {
                                    try_connect_sqlite(self, file.to_str().unwrap());
                                }
                            }
                            ui.add_space(8.0);
                            ui.label("or");
                            ui.add_space(8.0);
                            if ui.button("➕ create new project file").clicked() {
                                let maybe_file = FileDialog::new().set_directory("/").save_file();
                                if let Some(mut file) = maybe_file {
                                    if !file.ends_with(".fkrproj") {
                                        file.set_extension("fkrproj");
                                    }
                                    try_connect_sqlite(self, file.to_str().unwrap());
                                }
                            }
                        });
                    });
                });
                return;
            }

            ui.allocate_ui_with_layout(
                Vec2::new(0.0, 0.0),
                Layout::left_to_right(Align::BOTTOM),
                |ui| {
                    ui.label(
                        RichText::new("File Kraken")
                            .font(FontId::proportional(40.0))
                            .line_height(Some(40.0)),
                    );
                    ui.label(
                        RichText::from(String::from(" v") + VERSION)
                            .font(FontId::proportional(20.0)),
                    );
                },
            );

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.current_tab,
                    FileKrakenMainTabs::Locations,
                    RichText::new("Locations"),
                );
                ui.selectable_value(
                    &mut self.current_tab,
                    FileKrakenMainTabs::Files,
                    RichText::new("Files"),
                );
            });
            ui.separator();
            match self.current_tab {
                FileKrakenMainTabs::Locations => self.locations_tab(ui),
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
