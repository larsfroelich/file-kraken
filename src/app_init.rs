use crate::FileKrakenApp;

/// Run the app
pub fn app_init() -> eframe::Result {
    eframe::run_native(
        "File Kraken",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_maximized(true),
            ..Default::default()
        },
        Box::new(|cc| {
            cc.egui_ctx.set_zoom_factor(1.6);
            catppuccin_egui::set_theme(&cc.egui_ctx, catppuccin_egui::LATTE);
            Ok(Box::from(FileKrakenApp::new()))
        }),
    )
}