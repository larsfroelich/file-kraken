use std::panic::Location;
use egui::{Label, RichText, TextStyle, Ui, Window};
use egui_extras::{Column, TableBody, TableBuilder};
use log::{error, info, warn};
use rfd::FileDialog;
use crate::FileKrakenApp;
use crate::state::location::{FileKrakenLocation, FileKrakenLocationType};

#[derive(Default, PartialEq)]
pub struct LocationTabState {
    add_location_dialog_open: bool,
    add_location_path: String,
    add_location_type: FileKrakenLocationType,
    selected_location: Option<String>,
}

impl FileKrakenApp {
    pub fn locations_tab(
        &mut self,
        ui: &mut Ui) 
    {
        ui.horizontal_centered(|ui| {
            ui.columns(
                2,
                |cols| {
                    cols[0].with_layout(egui::Layout::top_down(egui::Align::Min), |ui| left_column(self, ui));
                    cols[1].with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        egui::Frame::none()
                            .fill(egui::Color32::LIGHT_GRAY)
                            .outer_margin(12.0)
                            .inner_margin(6.0)
                            .show(ui, |ui| {
                                ui.label("Details");
                            });
                    });
                }
            );
        });
        add_location_dialog_window(self, ui);
    }
}


fn add_location_dialog_window(_self: &mut FileKrakenApp, ui: &mut Ui) {
    let app_state = _self.app_state.clone();
    let mut add_location_dialog_open = _self.tab_state_locations.add_location_dialog_open;
    Window::new("Add location")
        .open(&mut _self.tab_state_locations.add_location_dialog_open)
        .show(ui.ctx(), |ui| {
            ui.label("Add a new location");
            ui.horizontal(|ui| {
                ui.label("Path:");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut _self.tab_state_locations.add_location_path);
                    ui.button("📁").on_hover_text("Browse for a folder").clicked().then(|| {
                        if let Some(folder) = FileDialog::new()
                            .add_filter("FileKraken Proj", &["fkrproj"])
                            .set_directory("/")
                            .pick_folder() {
                                _self.tab_state_locations.add_location_path = String::from(folder.to_string_lossy());
                            }
                    });
                });
            });
            ui.horizontal(|ui| {
                ui.label("Type:");
                ui.radio_value(&mut _self.tab_state_locations.add_location_type, FileKrakenLocationType::Normal, "Normal");
                ui.radio_value(&mut _self.tab_state_locations.add_location_type, FileKrakenLocationType::Preferred, "Preferred")
                    .on_hover_text("Preferred locations are where files are kept when duplicates are found");
                ui.radio_value(&mut _self.tab_state_locations.add_location_type, FileKrakenLocationType::Excluded, "Excluded")
                    .on_hover_text("Excluded locations are not scanned");
            });
            ui.vertical_centered_justified(|ui| {
                if ui.button("Add").clicked() {
                    add_location_dialog_open = false;
                    app_state.write().unwrap().add_location(FileKrakenLocation {
                        path: _self.tab_state_locations.add_location_path.clone(),
                        location_type: _self.tab_state_locations.add_location_type.clone(),
                    });
                    // add the location
                    error!("Adding location: {} {:?}", _self.tab_state_locations.add_location_path, _self.tab_state_locations.add_location_type);
                }
            });
        });
    _self.tab_state_locations.add_location_dialog_open &= add_location_dialog_open;
}

fn left_column(_self: &mut FileKrakenApp, ui: &mut Ui) {
    let app_state = _self.app_state.clone();
    
    ui.allocate_ui_with_layout(
        // leave 20 vertical space for the add button
        egui::vec2(ui.available_width(),ui.available_height()-20.0),
        egui::Layout::top_down(egui::Align::Min), |ui| {
        egui::Frame::none()
            .stroke(egui::Stroke::new(1.0, egui::Color32::DARK_GRAY))
            .outer_margin(12.0)
            .inner_margin(6.0)
            .show(ui, |ui| {
                TableBuilder::new(ui)
                    .sense(egui::Sense::click())
                    .column(Column::auto())
                    .column(Column::remainder())
                    .cell_layout(egui::Layout::top_down_justified(egui::Align::LEFT))
                    .header(25.0, |mut row| {
                        row.col(|ui| { ui.label(RichText::new(" ").strong()); });
                        row.col(|ui| { ui.label(RichText::new("Location path").strong()); });
                    })
                    .body(|mut body| {
                        for location in app_state.read().unwrap().get_locations_list_readonly().iter() {
                            table_row(&mut body, &location.path, &location.location_type, &mut _self.tab_state_locations.selected_location);
                        }
                    });
            });
    });
    ui.vertical_centered_justified(|ui| {
        ui.button("➕ add location").clicked().then(|| {
            _self.tab_state_locations.add_location_dialog_open = true;
        });
    });
}

// render a single row of the locations table
fn table_row(body: &mut TableBody, path: &str, location_type: &FileKrakenLocationType, selected_location: &mut Option<String>) {
    body.row(18.0, |mut row| {
        row.set_selected(
            selected_location
                .as_ref()
                .is_some_and(|x|x.eq(&String::from(path)))
        );
        row.col(|ui| {
            match location_type {
                FileKrakenLocationType::Preferred => { ui.add(Label::selectable(Label::new("⭐"), false) );},
                FileKrakenLocationType::Excluded => { ui.add(Label::selectable(Label::new("❌"), false) );},
                _ => {  }
            }
        });
        row.col(|ui| {
            if location_type == &FileKrakenLocationType::Excluded {
                ui.add(Label::selectable(Label::new(RichText::new(path).text_style(TextStyle::Monospace).strikethrough()), false) );
            }else {
                ui.add(Label::selectable(Label::new(RichText::new(path).text_style(TextStyle::Monospace)), false) );
            }
        }).1.clicked().then(|| {
            *selected_location = Some(path.to_string());
        });
    });
}