use crate::processing::find_duplicates::{
    delete_duplicate, find_file_duplicates, FindDuplicatesStateType,
};
use crate::state::duplicate::FileKrakenDuplicate;
use crate::state::AppState;
use crate::utils::ui_elements::{colored_box, unselectable_label};
use crate::FileKrakenApp;
use egui::{Color32, RichText, Ui};
use egui_extras::{Column, TableBuilder, TableRow};
use rfd::MessageDialogResult;
use std::ops::Deref;
use std::sync::Arc;
use std::thread;

impl FileKrakenApp {
    pub fn files_tab_duplicates(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            colored_box(ui, Color32::LIGHT_GRAY, |ui| {
                ui.label("File Duplicates");
            });
            colored_box(ui, Color32::TRANSPARENT, |ui| {
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
                            if ui.button("Find Duplicates").clicked() {
                                // new thread
                                let _app_state = self.app_state.clone();
                                thread::spawn(move || {
                                    find_file_duplicates(_app_state.clone());
                                });
                            }
                        }
                        FindDuplicatesStateType::Processing(message) => {
                            ui.label("Processing");
                            ui.spinner();
                            ui.label(": ");
                            ui.label(message);
                        }
                        FindDuplicatesStateType::Processed => {
                            ui.label("Finished");
                            if ui.button("Re-run search").clicked() {
                                // new thread
                                let _app_state = self.app_state.clone();
                                thread::spawn(move || {
                                    find_file_duplicates(_app_state.clone());
                                });
                            }
                        }
                    }
                });
                ui.separator();
                {
                    let duplicates = self
                        .app_state
                        .find_duplicates_processing
                        .duplicates
                        .read()
                        .unwrap();
                    ui.label(format!("Duplicates: {}", duplicates.len()));
                    let nr_eligible = duplicates
                        .iter()
                        .filter(|x| x.deletable_file.is_some())
                        .count();
                    ui.horizontal(|ui| {
                        ui.label(format!("Eligible for deletion: {}", nr_eligible));
                        if nr_eligible > 1
                            && self
                                .app_state
                                .find_duplicates_processing
                                .state
                                .read()
                                .unwrap()
                                .eq(&FindDuplicatesStateType::Processed)
                        {
                            ui.add_space(5.0);
                            if ui.button("Delete all eligible duplicates").clicked() {
                                if rfd::MessageDialog::new()
                                    .set_title("Delete all eligible duplicates")
                                    .set_description(
                                        "Are you sure you want to delete all eligible duplicates?",
                                    )
                                    .set_buttons(rfd::MessageButtons::YesNo)
                                    .show()
                                    .eq(&MessageDialogResult::Yes)
                                {
                                    let _duplicates = self
                                        .app_state
                                        .find_duplicates_processing
                                        .duplicates
                                        .read()
                                        .unwrap()
                                        .clone();
                                    let _app_state = self.app_state.clone();
                                    thread::spawn(move || {
                                        let total_nr_duplicates = _duplicates.len();
                                        let mut nr_deleted = 0;
                                        for duplicate in _duplicates.iter() {
                                            {
                                                *_app_state
                                                    .find_duplicates_processing
                                                    .state
                                                    .write()
                                                    .unwrap() =
                                                    FindDuplicatesStateType::Processing(format!(
                                                        "Deleting eligible duplicates ... ({}/{})",
                                                        nr_deleted, total_nr_duplicates
                                                    ));
                                            }
                                            if let Some(_) = &duplicate.deletable_file {
                                                delete_duplicate(&_app_state, duplicate);
                                            }
                                            nr_deleted += 1;
                                        }
                                        *_app_state
                                            .find_duplicates_processing
                                            .state
                                            .write()
                                            .unwrap() = FindDuplicatesStateType::Processed;
                                    });
                                }
                            }
                        }
                    });
                }
                let (eligible_duplicates, ineligible_duplicates) = {
                    let duplicates = self
                        .app_state
                        .find_duplicates_processing
                        .duplicates
                        .read()
                        .unwrap();
                    (
                        duplicates
                            .iter()
                            .filter(|x| x.deletable_file.is_some())
                            .cloned()
                            .collect::<Vec<_>>(),
                        duplicates
                            .iter()
                            .filter(|x| x.deletable_file.is_none())
                            .cloned()
                            .collect::<Vec<_>>(),
                    )
                };

                egui::Frame::none()
                    .stroke(egui::Stroke::new(1.0, egui::Color32::DARK_GRAY))
                    .outer_margin(12.0)
                    .inner_margin(6.0)
                    .show(ui, |ui| {
                        let available_width = ui.available_width();
                        TableBuilder::new(ui)
                            .sense(egui::Sense::click())
                            .column(Column::exact(25.0))
                            .column(Column::exact(15.0))
                            .column(Column::exact(available_width / 2.0 - 40.0))
                            .column(Column::exact(available_width / 2.0 - 40.0))
                            .cell_layout(egui::Layout::top_down_justified(egui::Align::LEFT))
                            .header(25.0, |mut row| {
                                row.col(|ui| {
                                    ui.label(RichText::new(" ").strong());
                                });
                                row.col(|ui| {
                                    ui.label(RichText::new(" ").strong());
                                });
                                row.col(|ui| {
                                    ui.label(RichText::new("Location path 1").strong());
                                });
                                row.col(|ui| {
                                    ui.label(RichText::new("Location path 2").strong());
                                });
                            })
                            .body(|body| {
                                body.rows(
                                    18.0,
                                    eligible_duplicates.len() + ineligible_duplicates.len(),
                                    |mut row| {
                                        if let Some(duplicate) = eligible_duplicates
                                            .iter()
                                            .chain(ineligible_duplicates.iter())
                                            .nth(row.index())
                                        {
                                            table_row(&self.app_state, &mut row, &duplicate);
                                        }
                                    },
                                );
                            });
                    });
            });
        });
    }
}

fn table_row(app_state: &Arc<AppState>, row: &mut TableRow, duplicate: &FileKrakenDuplicate) {
    let color = if duplicate.deletable_file.is_some() {
        Color32::from_rgb(0, 0, 0)
    } else {
        Color32::from_rgb(78, 78, 78)
    };

    row.col(|ui| {
        if duplicate.deletable_file.is_some() {
            if ui.button("🗑️").clicked() {
                if rfd::MessageDialog::new()
                    .set_title("Delete file?")
                    .set_description(format!(
                        "Are you sure you want to delete the file \"{}\"?",
                        duplicate.deletable_file.as_ref().unwrap().path.to_string()
                    ))
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show()
                    .eq(&MessageDialogResult::Yes)
                {
                    delete_duplicate(app_state, duplicate);
                }
            }
        }
    });
    row.col(|ui| {
        if duplicate.deletable_file.is_some() {
            unselectable_label(ui, RichText::new("🆗").color(Color32::DARK_BLUE));
        } else {
            unselectable_label(ui, RichText::new("❌").color(Color32::DARK_RED));
        }
    });
    row.col(|ui| {
        unselectable_label(
            ui,
            RichText::new(duplicate.other_files.get(0).unwrap().path.to_string()).color(color),
        );
    });
    row.col(|ui| {
        if let Some(deletable_file) = &duplicate.deletable_file {
            unselectable_label(
                ui,
                RichText::new(deletable_file.path.to_string())
                    .color(color)
                    .strikethrough(),
            );
        } else {
            unselectable_label(
                ui,
                RichText::new(duplicate.other_files.get(1).unwrap().path.to_string()).color(color),
            );
        }
    });
}
