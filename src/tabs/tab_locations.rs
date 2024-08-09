use crate::processing::scan::scan_location_files;
use crate::state::location::{FileKrakenLocationState, FileKrakenLocationType};
use crate::state::AppState;
use crate::utils::ui_elements::colored_box;
use crate::FileKrakenApp;
use egui::{Label, RichText, TextStyle, Ui, Window};
use egui_extras::{Column, TableBody, TableBuilder};
use rfd::FileDialog;
use std::sync::Arc;
use std::thread;

#[derive(Default, PartialEq)]
pub struct LocationTabState {
    add_location_dialog_open: bool,
    add_location_path: String,
    add_location_type: FileKrakenLocationType,
    selected_location: Option<String>,
    modify_location_dialog_open: bool,
    modify_location_path: String,
    modify_location_type: FileKrakenLocationType,
}

impl FileKrakenApp {
    pub fn locations_tab(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            ui.columns(2, |cols| {
                cols[0].with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    left_column(self, ui)
                });
                cols[1].with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    right_column(self, ui)
                });
            });
        });
        add_location_dialog_window(self, ui);
        modify_location_dialog_window(self, ui);
    }
}

fn right_column(_self: &mut FileKrakenApp, ui: &mut Ui) {
    if let Some(selected_location) = &_self.tab_state_locations.selected_location {
        if let Some(location) = _self.app_state.get_location_clone(selected_location) {
            colored_box(ui, egui::Color32::LIGHT_GRAY, |ui| {
                ui.label("Location details:");
            });
            colored_box(ui, egui::Color32::TRANSPARENT, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("Path:");
                        if location.location_state == FileKrakenLocationState::Deleting {
                            ui.label(
                                RichText::new(&location.path)
                                    .text_style(TextStyle::Monospace)
                                    .strikethrough(),
                            );
                        } else {
                            ui.label(
                                RichText::new(&location.path).text_style(TextStyle::Monospace),
                            );
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Type:");
                        ui.label(match location.location_type {
                            FileKrakenLocationType::Normal => "Normal",
                            FileKrakenLocationType::Preferred => "Preferred",
                            FileKrakenLocationType::Excluded => "Excluded",
                        });
                        ui.button("📝")
                            .on_hover_text("Modify the location type")
                            .clicked()
                            .then(|| {
                                _self.tab_state_locations.modify_location_dialog_open = true;
                                _self.tab_state_locations.modify_location_path =
                                    location.path.clone();
                                _self.tab_state_locations.modify_location_type =
                                    location.location_type.clone();
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Files:");
                        ui.label(
                            _self
                                .app_state
                                .get_files_by_location(&location.path)
                                .unwrap()
                                .read()
                                .unwrap()
                                .len()
                                .to_string(),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Parent location:");
                        ui.label(match &location.parent_location_path {
                            Some(x) => x,
                            None => "None",
                        });
                    });
                });
            });

            colored_box(ui, egui::Color32::LIGHT_GRAY, |ui| {
                ui.label("Location status:");
            });
            colored_box(ui, egui::Color32::TRANSPARENT, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("State:");
                        ui.label(match location.location_state {
                            FileKrakenLocationState::Unscanned => "Not scanned",
                            FileKrakenLocationState::PartialScanned => "Partially scanned",
                            FileKrakenLocationState::Scanned => "Scanned",
                            FileKrakenLocationState::Scanning => "Scanning ...",
                            FileKrakenLocationState::Deleting => "⚠️ Deleting ...",
                        });
                        if location.location_state == FileKrakenLocationState::Scanning
                            || location.location_state == FileKrakenLocationState::Deleting
                        {
                            ui.spinner();
                        }
                    });

                    if location.location_state != FileKrakenLocationState::Scanning
                        && location.location_state != FileKrakenLocationState::Deleting
                    {
                        ui.button("Scan location")
                            .on_hover_text("Scan the location for files")
                            .clicked()
                            .then(|| {
                                // new thread
                                let _app_state = _self.app_state.clone();
                                let _path = location.path.clone();
                                thread::spawn(move || {
                                    scan_location_files(_app_state.clone(), &_path);
                                });
                            });
                    }
                });
            });
        } else {
            colored_box(ui, egui::Color32::LIGHT_GRAY, |ui| {
                ui.vertical_centered_justified(|ui| ui.label("Selected location not found"));
            });
        }
    } else {
        colored_box(ui, egui::Color32::LIGHT_GRAY, |ui| {
            ui.vertical_centered_justified(|ui| ui.label("No location selected"));
        });
    }
}

fn modify_location_dialog_window(_self: &mut FileKrakenApp, ui: &mut Ui) {
    let app_state = _self.app_state.clone();
    let mut modify_location_dialog_open = _self.tab_state_locations.modify_location_dialog_open;
    Window::new("Modify location")
        .open(&mut _self.tab_state_locations.modify_location_dialog_open)
        .show(ui.ctx(), |ui| {
            ui.label("Modify location");
            ui.horizontal(|ui| {
                ui.label("Path:");
                ui.monospace(&_self.tab_state_locations.modify_location_path);
            });
            ui.horizontal(|ui| {
                ui.label("Type:");
                ui.radio_value(
                    &mut _self.tab_state_locations.modify_location_type,
                    FileKrakenLocationType::Normal,
                    "Normal",
                );
                ui.radio_value(
                    &mut _self.tab_state_locations.modify_location_type,
                    FileKrakenLocationType::Preferred,
                    "Preferred",
                )
                    .on_hover_text(
                        "Preferred locations are where files are kept when duplicates are found",
                    );
                ui.radio_value(
                    &mut _self.tab_state_locations.modify_location_type,
                    FileKrakenLocationType::Excluded,
                    "Excluded",
                )
                    .on_hover_text("Excluded locations are not scanned");
            });
            ui.vertical_centered_justified(|ui| {
                if ui.button("Modify").clicked() {
                    modify_location_dialog_open = false;
                    app_state.modify_location_type(
                        &_self.tab_state_locations.modify_location_path.clone(),
                        _self.tab_state_locations.modify_location_type.clone(),
                    );
                }
                if ui.button("Cancel").clicked() {
                    modify_location_dialog_open = false;
                }
            });
        });
    _self.tab_state_locations.modify_location_dialog_open &= modify_location_dialog_open;
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
                    ui.button("📁")
                        .on_hover_text("Browse for a folder")
                        .clicked()
                        .then(|| {
                            if let Some(folder) = FileDialog::new()
                                .add_filter("FileKraken Proj", &["fkrproj"])
                                .set_directory("/")
                                .pick_folder()
                            {
                                _self.tab_state_locations.add_location_path =
                                    String::from(folder.to_string_lossy());
                            }
                        });
                });
            });
            ui.horizontal(|ui| {
                ui.label("Type:");
                ui.radio_value(
                    &mut _self.tab_state_locations.add_location_type,
                    FileKrakenLocationType::Normal,
                    "Normal",
                );
                ui.radio_value(
                    &mut _self.tab_state_locations.add_location_type,
                    FileKrakenLocationType::Preferred,
                    "Preferred",
                )
                    .on_hover_text(
                        "Preferred locations are where files are kept when duplicates are found",
                    );
                ui.radio_value(
                    &mut _self.tab_state_locations.add_location_type,
                    FileKrakenLocationType::Excluded,
                    "Excluded",
                )
                    .on_hover_text("Excluded locations are not scanned");
            });
            ui.vertical_centered_justified(|ui| {
                if ui.button("Add").clicked() {
                    add_location_dialog_open = false;

                    app_state.add_location(
                        true,
                        &_self.tab_state_locations.add_location_path,
                        &_self.tab_state_locations.add_location_type,
                        &FileKrakenLocationState::Unscanned,
                    );

                    // reset the dialog
                    _self.tab_state_locations.add_location_path = String::new();
                    _self.tab_state_locations.add_location_type = FileKrakenLocationType::Normal;
                }
            });
        });
    _self.tab_state_locations.add_location_dialog_open &= add_location_dialog_open;
}

fn left_column(_self: &mut FileKrakenApp, ui: &mut Ui) {
    let app_state = _self.app_state.clone();

    ui.allocate_ui_with_layout(
        // leave 20 vertical space for the add button
        egui::vec2(ui.available_width(), ui.available_height() - 20.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            egui::Frame::none()
                .stroke(egui::Stroke::new(1.0, egui::Color32::DARK_GRAY))
                .outer_margin(12.0)
                .inner_margin(6.0)
                .show(ui, |ui| {
                    TableBuilder::new(ui)
                        .sense(egui::Sense::click())
                        .column(Column::auto())
                        .column(Column::auto())
                        .column(Column::remainder())
                        .cell_layout(egui::Layout::top_down_justified(egui::Align::LEFT))
                        .header(25.0, |mut row| {
                            row.col(|ui| {
                                ui.label(RichText::new(" ").strong());
                            });
                            row.col(|ui| {
                                ui.label(RichText::new("Location path").strong());
                            });
                        })
                        .body(|mut body| {
                            for location in app_state.get_locations_list_readonly().iter() {
                                table_row(
                                    &app_state,
                                    &mut body,
                                    &location.path,
                                    &location.location_type,
                                    &location.location_state,
                                    &mut _self.tab_state_locations.selected_location,
                                );
                            }
                        });
                });
        },
    );
    ui.vertical_centered_justified(|ui| {
        ui.button("➕ add location").clicked().then(|| {
            _self.tab_state_locations.add_location_dialog_open = true;
        });
    });
}

// render a single row of the locations table
fn table_row(
    app_state: &Arc<AppState>,
    body: &mut TableBody,
    path: &str,
    location_type: &FileKrakenLocationType,
    location_state: &FileKrakenLocationState,
    selected_location: &mut Option<String>,
) {
    body.row(18.0, |mut row| {
        row.set_selected(
            selected_location
                .as_ref()
                .is_some_and(|x| x.eq(&String::from(path))),
        );

        row.col(|ui| {
            if ui.button("🗑️").clicked()
                && rfd::MessageDialog::new()
                .set_title("Remove location")
                .set_description(&format!(
                    "Are you sure you want to remove the location: \"{}\"?",
                    path
                ))
                .set_buttons(rfd::MessageButtons::YesNo)
                .show()
                .eq(&rfd::MessageDialogResult::Yes)
            {
                let _app_state = app_state.clone();
                let _path = path.to_string();
                thread::spawn(move || {
                    _app_state.remove_location(true, &_path);
                });
            }
        });
        row.col(|ui| match location_type {
            FileKrakenLocationType::Preferred => {
                ui.add(Label::selectable(Label::new("⭐"), false));
            }
            FileKrakenLocationType::Excluded => {
                ui.add(Label::selectable(Label::new("❌"), false));
            }
            _ => {}
        });
        row.col(|ui| {
            if location_type == &FileKrakenLocationType::Excluded
                || location_state == &FileKrakenLocationState::Deleting
            {
                ui.add(Label::selectable(
                    Label::new(
                        RichText::new(path)
                            .text_style(TextStyle::Monospace)
                            .strikethrough(),
                    ),
                    false,
                ));
            } else {
                ui.add(Label::selectable(
                    Label::new(RichText::new(path).text_style(TextStyle::Monospace)),
                    false,
                ));
            }
        })
            .1
            .clicked()
            .then(|| {
                *selected_location = Some(path.to_string());
            });
    });
}
