use std::panic::Location;
use egui::RichText;
use egui_extras::{Column, TableBuilder};
use crate::FileKrakenApp;

impl FileKrakenApp {
    pub fn locations_tab(&mut self, ui: &mut egui::Ui, locations_list: Option<&Vec<Location>>) {
        ui.horizontal_centered(|ui| {
            ui.columns(
                2,
                |cols| {
                    cols[0].with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        egui::Frame::none()
                            .fill(egui::Color32::LIGHT_GRAY)
                            .show(ui, |ui| {
                                TableBuilder::new(ui)
                                    .column(Column::auto())
                                    .column(Column::remainder())
                                    .cell_layout(egui::Layout::top_down_justified(egui::Align::LEFT))
                                    .header(25.0, |mut row| {
                                        row.col(|ui| { ui.label(RichText::new("⭐").strong()); });
                                        row.col(|ui| { ui.label(RichText::new("Location path").strong()); });
                                    })
                                    .body(|mut body| {
                                        for _ in 0..4 {
                                            body.row(18.0, |mut row| {
                                                row.col(|ui| {
                                                    ui.label("⭐");
                                                });
                                                row.col(|ui| {
                                                    ui.monospace("/test/path/mnt/one");
                                                });
                                            });
                                        }
                                        for _ in 0..30 {
                                            body.row(18.0, |mut row| {
                                                row.col(|ui| {
                                                    ui.label(" ");
                                                });
                                                row.col(|ui| {
                                                    ui.monospace("/test/path/mnt/one");
                                                });
                                            });
                                        }
                                    });
                            })
                        
                    });

                    cols[1].with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {

                    });
                }
            );

        });
    }
}