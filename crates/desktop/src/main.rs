//! Chai Desktop — application entry.

mod app;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Chai",
        options,
        Box::new(|cc| Box::new(app::ChaiApp::new(cc))),
    )
}
