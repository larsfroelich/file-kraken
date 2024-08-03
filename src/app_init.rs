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
            cc.egui_ctx.set_zoom_factor(2.0);
            catppuccin_egui::set_theme(&cc.egui_ctx, catppuccin_egui::LATTE);
            Ok(Box::<FileKrakenApp>::default())
        }),
    )
}