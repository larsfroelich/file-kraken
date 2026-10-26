#![allow(clippy::collapsible_if)]
use crate::processing::find_duplicates::{
    delete_duplicate, find_file_duplicates, get_duplicates_processing_state,
    set_processing_message, FindDuplicatesStateType,
};
use crate::state::duplicate::FileKrakenDuplicate;
use crate::state::AppState;
use crate::utils::size_unit::SizeUnit;
use crate::utils::ui_elements::{colored_box, unselectable_label};
use crate::FileKrakenApp;
use egui::{Color32, RichText, Ui};
use egui_extras::{Column, TableBuilder, TableRow};
use rfd::MessageDialogResult;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::thread;

impl FileKrakenApp {
    pub fn files_tab_duplicates(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            colored_box(
                ui,
                Color32::TRANSPARENT,
                egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
                |ui| {
                    ui.label("File Duplicates");
                },
            );
            colored_box(ui, Color32::TRANSPARENT, egui::Stroke::NONE, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Status: ");
                    match self.app_state.find_duplicates_processing.state.read().unwrap().deref() {
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

                    let is_processing = matches!(
                        *self.app_state.find_duplicates_processing.state.read().unwrap(),
                        FindDuplicatesStateType::Processing(_)
                    );
                    ui.add_enabled_ui(!is_processing, |ui| {
                        ui.add_space(20.0);
                        ui.label("Min file size:");
                        let mut min_size_input = self
                            .app_state
                            .find_duplicates_processing
                            .min_file_size_input
                            .read()
                            .unwrap()
                            .clone();
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut min_size_input)
                                    .desired_width(100.0),
                            )
                            .changed()
                        {
                            // only allow numbers
                            if min_size_input.chars().all(|c| c.is_ascii_digit()) {
                                *self
                                    .app_state
                                    .find_duplicates_processing
                                    .min_file_size_input
                                    .write()
                                    .unwrap() = min_size_input.clone();
                                self.app_state
                                    .set_setting("min_file_size_input", &min_size_input);
                            }
                        }

                        let mut min_size_unit = *self
                            .app_state
                            .find_duplicates_processing
                            .min_file_size_unit
                            .read()
                            .unwrap();
                        egui::ComboBox::from_id_source("min_file_size_unit")
                            .selected_text(min_size_unit.to_string())
                            .show_ui(ui, |ui| {
                                for unit in SizeUnit::all() {
                                    if ui
                                        .selectable_value(&mut min_size_unit, unit, unit.to_string())
                                        .clicked()
                                    {
                                        *self
                                            .app_state
                                            .find_duplicates_processing
                                            .min_file_size_unit
                                            .write()
                                            .unwrap() = unit;
                                        self.app_state
                                            .set_setting("min_file_size_unit", &unit.to_string());
                                    }
                                }
                            });

                        ui.add_space(20.0);
                        let mut include_same = *self
                            .app_state
                            .find_duplicates_processing
                            .include_same_location_duplicates
                            .read()
                            .unwrap();
                        if ui
                            .checkbox(&mut include_same, "Include same-location duplicates")
                            .clicked()
                        {
                            *self
                                .app_state
                                .find_duplicates_processing
                                .include_same_location_duplicates
                                .write()
                                .unwrap() = include_same;
                            self.app_state.set_setting(
                                "include_same_location_duplicates",
                                &include_same.to_string(),
                            );
                        }

                        ui.add_space(20.0);
                        let mut scan_zips = *self
                            .app_state
                            .find_duplicates_processing
                            .scan_inside_zip_files
                            .read()
                            .unwrap();
                        if ui
                            .checkbox(&mut scan_zips, "Scan inside zip files")
                            .clicked()
                        {
                            *self
                                .app_state
                                .find_duplicates_processing
                                .scan_inside_zip_files
                                .write()
                                .unwrap() = scan_zips;
                            self.app_state.set_setting(
                                "scan_inside_zip_files",
                                &scan_zips.to_string(),
                            );
                        }
                    });
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
                            let mut has_zip_duplicates = false;
                            {
                                let duplicates = self.app_state.find_duplicates_processing.duplicates.read().unwrap();
                                for dup in duplicates.iter() {
                                    if let Some(deletable) = &dup.deletable_file {
                                        if deletable.path.contains("::") {
                                            has_zip_duplicates = true;
                                            break;
                                        }
                                    }
                                }
                            }

                            let btn_text = if has_zip_duplicates {
                                "Delete all eligible duplicates (One per ZIP per pass)"
                            } else {
                                "Delete all eligible duplicates"
                            };

                            if ui.button(btn_text).clicked() {
                                if rfd::MessageDialog::new()
                                    .set_title("Delete all eligible duplicates")
                                    .set_description(
                                        "Are you sure you want to delete all eligible duplicates?",
                                    )
                                    .set_buttons(rfd::MessageButtons::YesNo)
                                    .show()
                                    .eq(&MessageDialogResult::Yes)
                                {
                                    let mut _duplicates = self
                                        .app_state
                                        .find_duplicates_processing
                                        .duplicates
                                        .read()
                                        .unwrap()
                                        .clone();

                                    // Check for large zip files
                                    let mut has_large_zip = false;
                                    for dup in &_duplicates {
                                        if let Some(deletable) = &dup.deletable_file {
                                            if let Some((zip_path, _)) = deletable.path.split_once("::") {
                                                if let Ok(metadata) = std::fs::metadata(zip_path) {
                                                    if metadata.len() > 500 * 1024 * 1024 {
                                                        has_large_zip = true;
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    if has_large_zip {
                                        let skip = rfd::MessageDialog::new()
                                            .set_title("Large Zip Files Detected")
                                            .set_description("Some duplicates are inside zip files larger than 500MB. Modifying them can be slow and use lots of disk space. Skip deleting files in large zips?")
                                            .set_buttons(rfd::MessageButtons::YesNo)
                                            .show()
                                            .eq(&MessageDialogResult::Yes);

                                        if skip {
                                            _duplicates.retain(|dup| {
                                                if let Some(deletable) = &dup.deletable_file {
                                                    if let Some((zip_path, _)) = deletable.path.split_once("::") {
                                                        if let Ok(metadata) = std::fs::metadata(zip_path) {
                                                            if metadata.len() > 500 * 1024 * 1024 {
                                                                return false;
                                                            }
                                                        }
                                                    }
                                                }
                                                true
                                            });
                                        }
                                    }

                                    let _app_state = self.app_state.clone();
                                    let total_nr_eligible = _duplicates.iter().filter(|x| x.deletable_file.is_some()).count();
                                    thread::spawn(move || {
                                        let mut nr_deleted = 0;
                                        set_processing_message(&_app_state, format!(
                                            "Deleting eligible duplicates ... {:.2}% ({}/{})",
                                            (nr_deleted as f64 / total_nr_eligible as f64) * 100.0,
                                            nr_deleted, total_nr_eligible
                                        ));

                                        // Group by ZIP archive to minimize rewrites and index invalidations
                                        // A simple grouping strategy: process non-zip files normally, process zip files later
                                        let mut normal_deletes = Vec::new();
                                        let mut zip_deletes = Vec::new();

                                        for duplicate in _duplicates.iter() {
                                            if let Some(deletable) = &duplicate.deletable_file {
                                                if deletable.path.contains("::") {
                                                    zip_deletes.push(duplicate.clone());
                                                } else {
                                                    normal_deletes.push(duplicate.clone());
                                                }
                                            }
                                        }

                                        for duplicate in normal_deletes.iter() {
                                            set_processing_message(&_app_state, format!(
                                                "Deleting eligible duplicates ... {:.2}% ({}/{})",
                                                (nr_deleted as f64 / total_nr_eligible as f64) * 100.0,
                                                nr_deleted, total_nr_eligible
                                            ));
                                            delete_duplicate(&_app_state, duplicate);
                                            nr_deleted += 1;
                                        }

                                        // For ZIP files, since one deletion invalidates remaining indexes,
                                        // we now process them in a loop where we process one per archive, trigger a rescan/rehash of that archive,
                                        // and then fetch the fresh duplicate state if more remain.
                                        // However, to keep it simple within this thread without recursive locking,
                                        // we'll just process ONE zip member per archive as before, BUT we will update the UI message
                                        // to make it explicit that multiple clicks may be required, or we just let it trigger the re-run automatically.
                                        // Actually, since we trigger find_file_duplicates at the end, it WILL find the remaining ones automatically
                                        // but the user would have to click again. Let's make it more robust.

                                        let mut deleted_zips = std::collections::HashSet::new();
                                        let mut had_zip = false;

                                        // We only do one pass of zips here. The UI will show the remaining ones after the auto-rerun.
                                        // We should change the button text or add a label explaining this.
                                        // For now, let's keep the single-pass behavior and we will add a note in the UI below.
                                        for duplicate in zip_deletes.iter() {
                                            if let Some(deletable) = &duplicate.deletable_file {
                                                if let Some((zip_path, _)) = deletable.path.split_once("::") {
                                                    if !deleted_zips.contains(zip_path) {
                                                        had_zip = true;
                                                        deleted_zips.insert(zip_path.to_string());

                                                        set_processing_message(&_app_state, format!(
                                                            "Deleting eligible duplicates ... {:.2}% ({}/{})",
                                                            (nr_deleted as f64 / total_nr_eligible as f64) * 100.0,
                                                            nr_deleted, total_nr_eligible
                                                        ));

                                                        delete_duplicate(&_app_state, duplicate);
                                                        nr_deleted += 1;
                                                    }
                                                }
                                            }
                                        }

                                        if had_zip {
                                            // Rescan the locations that had zips modified to ensure fresh member state
                                            for zip_path in deleted_zips.iter() {
                                                if let Some(parent) = crate::utils::parent_path::get_longest_parent_path(zip_path, _app_state.get_locations_list_readonly().iter()) {
                                                    crate::processing::scan::scan_location_files(_app_state.clone(), &parent);
                                                }
                                            }
                                            crate::processing::find_duplicates::find_file_duplicates(_app_state.clone());
                                        } else {
                                            *get_duplicates_processing_state(&_app_state)
                                                .deref_mut()
                                                = FindDuplicatesStateType::Processed;
                                        }
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
                        TableBuilder::new(ui)
                            .sense(egui::Sense::click())
                            .striped(true)
                            .column(Column::exact(25.0))
                            .column(Column::exact(15.0))
                            .column(Column::remainder())
                            .column(Column::remainder())
                            .column(Column::auto())
                            .column(Column::auto())
                            .column(Column::auto())
                            .column(Column::auto())
                            .cell_layout(egui::Layout::top_down_justified(egui::Align::LEFT))
                            .header(25.0, |mut row| {
                                row.col(|ui| {
                                    ui.label(RichText::new(" ").strong());
                                });
                                row.col(|ui| {
                                    ui.label(RichText::new(" ").strong());
                                });
                                row.col(|ui| {
                                    ui.label(RichText::new("Location 1").strong());
                                });
                                row.col(|ui| {
                                    ui.label(RichText::new("Location 2").strong());
                                });
                                row.col(|ui| {
                                    ui.label(RichText::new("Type").strong());
                                });
                                row.col(|ui| {
                                    ui.label(RichText::new("Size").strong());
                                });
                                row.col(|ui| {
                                    ui.label(RichText::new("Created").strong());
                                });
                                row.col(|ui| {
                                    ui.label(RichText::new("Modified").strong());
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
                                            table_row(&self.app_state, &mut row, duplicate);
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
        if let Some(deletable_file) = &duplicate.deletable_file {            if ui.button("🗑️").clicked()
                && rfd::MessageDialog::new()
                    .set_title("Delete file?")
                    .set_description(format!(
                        "Are you sure you want to delete the file \"{}\"?",
                        deletable_file.path
                    ))
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show()
                    .eq(&MessageDialogResult::Yes)
            {
                let mut proceed = true;
                if let Some((zip_path, _)) = deletable_file.path.split_once("::") {
                    if let Ok(metadata) = std::fs::metadata(zip_path) {
                        if metadata.len() > 500 * 1024 * 1024 {
                            proceed = rfd::MessageDialog::new()
                                .set_title("Large Zip File Detected")
                                .set_description("This file is inside a zip file larger than 500MB. Modifying it can be slow and use lots of disk space. Proceed?")
                                .set_buttons(rfd::MessageButtons::YesNo)
                                .show()
                                .eq(&MessageDialogResult::Yes);
                        }
                    }
                }
                if proceed {
                    let has_zip = deletable_file.path.contains("::");
                    let zip_path = if has_zip {
                        deletable_file.path.split_once("::").map(|(z, _)| z.to_string())
                    } else { None };

                    if has_zip {
                        // Lock the UI before making the first delete to prevent clicking stale indexes
                        *crate::processing::find_duplicates::get_duplicates_processing_state(app_state)
                            .deref_mut() = FindDuplicatesStateType::Processing("Reindexing zip archive...".to_string());
                    }

                    delete_duplicate(app_state, duplicate);

                    if let Some(zp) = zip_path {
                        // Restart duplicate search automatically to refresh stale indexes in the UI
                        // Also rescan the zip to ensure members are correctly updated
                        let _app_state = app_state.clone();
                        std::thread::spawn(move || {
                            if let Some(parent) = crate::utils::parent_path::get_longest_parent_path(&zp, _app_state.get_locations_list_readonly().iter()) {
                                // Full location rescan ensures zip members are freshly enumerated
                                crate::processing::scan::scan_location_files(_app_state.clone(), &parent);
                            }
                            crate::processing::find_duplicates::find_file_duplicates(_app_state);
                        });
                    }
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
            RichText::new(duplicate.other_files.first().unwrap().path.to_string()).color(color),
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
    let metadata_file = duplicate
        .deletable_file
        .as_ref()
        .or_else(|| duplicate.other_files.first())
        .unwrap();
    row.col(|ui| {
        unselectable_label(
            ui,
            RichText::new(duplicate.duplicate_type.to_string()).color(color),
        );
    });
    row.col(|ui| {
        unselectable_label(
            ui,
            RichText::new(metadata_file.file_len.to_string()).color(color),
        );
    });
    row.col(|ui| {
        unselectable_label(
            ui,
            RichText::new(metadata_file.time_created.to_string()).color(color),
        );
    });
    row.col(|ui| {
        unselectable_label(
            ui,
            RichText::new(metadata_file.time_modified.to_string()).color(color),
        );
    });
}
