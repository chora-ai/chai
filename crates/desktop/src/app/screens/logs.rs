use eframe::egui;

use crate::app::ChaiApp;
use crate::app::state::logs::log_buffer;

pub fn ui_logs_screen(_app: &ChaiApp, ui: &mut egui::Ui) {
    ui.add_space(24.0);
    ui.heading("Logs");
    ui.add_space(ChaiApp::SCREEN_TITLE_BOTTOM_SPACING);

    let lines: Vec<String> = log_buffer()
        .lock()
        .map(|b| b.iter().cloned().collect())
        .unwrap_or_default();

    let available = ui.available_height();
    let scroll_height = (available - ChaiApp::SCREEN_FOOTER_SPACING).max(0.0);
    egui::ScrollArea::vertical()
        .max_height(scroll_height)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for line in &lines {
                ui.label(
                    egui::RichText::new(line.as_str()).family(egui::FontFamily::Monospace),
                );
            }
            if lines.is_empty() {
                ui.label("No log output yet.");
            }
        });
    ui.add_space(ChaiApp::SCREEN_FOOTER_SPACING);
}

