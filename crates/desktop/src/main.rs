//! Chai Desktop — application entry.

mod app;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_maximized(true),
        ..Default::default()
    };
    eframe::run_native(
        "Chai",
        options,
        Box::new(|cc| Box::new(app::ChaiApp::new(cc))),
    )
}
