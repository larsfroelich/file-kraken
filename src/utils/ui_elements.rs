use egui::Ui;

pub fn colored_box(ui: &mut Ui, color: egui::Color32, f: impl FnOnce(&mut Ui)) {
    egui::Frame::none()
        .fill(color)
        .outer_margin(12.0)
        .inner_margin(6.0)
        .show(ui, f);
}
