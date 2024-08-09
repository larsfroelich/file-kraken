use egui::{Label, Ui, WidgetText};

pub fn colored_box(ui: &mut Ui, color: egui::Color32, f: impl FnOnce(&mut Ui)) {
    egui::Frame::none()
        .fill(color)
        .outer_margin(12.0)
        .inner_margin(6.0)
        .show(ui, f);
}

pub fn unselectable_label(ui: &mut Ui, text: impl Into<WidgetText>) {
    ui.add(Label::selectable(Label::new(text).truncate(), false));
}
