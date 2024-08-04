use std::panic::Location;
use egui::{Label, RichText, TextStyle};
use egui_extras::{Column, TableBody, TableBuilder};
use crate::FileKrakenApp;
use crate::location::FileKrakenLocationType;

impl FileKrakenApp {
    pub fn locations_tab(&mut self, ui: &mut egui::Ui, locations_list: Option<&Vec<Location>>) {
        fn table_row(body: &mut TableBody, path: &str, location_type: &FileKrakenLocationType, row_index: u32, selected_location: &mut Option<u32>) {
            body.row(18.0, |mut row| {
                row.set_selected(selected_location == &Some(row_index));
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
                    *selected_location = Some(row_index);
                });
            });
        }

        ui.horizontal_centered(|ui| {
            ui.columns(
                2,
                |cols| {
                    cols[0].with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
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
                                        for i in 0..4 {
                                            table_row(&mut body, "/test/path/mnt/one", &FileKrakenLocationType::Preferred , i, &mut self.selected_location);
                                        }
                                        table_row(&mut body, "/test/path/mnt/one", &FileKrakenLocationType::Excluded , 4, &mut self.selected_location);
                                        for i in 0..30 {
                                            table_row(&mut body, "/test/path/mnt/one", &FileKrakenLocationType::Normal , 5+i, &mut self.selected_location);
                                        }
                                    });
                            })
                        
                    });

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
    }
}