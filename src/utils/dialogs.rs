use log::error;

pub fn error_dialog(msg: &str) {
    error!("{}", msg);
    let dialog = rfd::MessageDialog::new()
        .set_title("Error")
        .set_description(msg)
        .set_buttons(rfd::MessageButtons::Ok);
    dialog.show();
}
