use eframe::egui;

use crate::app::state::logs::log_buffer;
use crate::app::ChaiApp;

pub fn ui_logs_screen(_app: &ChaiApp, ui: &mut egui::Ui) {
    let lines: Vec<String> = log_buffer()
        .lock()
        .map(|b| b.iter().cloned().collect())
        .unwrap_or_default();

    crate::app::ui_screen(
        ui,
        "Logs",
        Some("Values below are loaded from an in-memory buffer."),
        |ui| {
            let available = ui.available_height();
            let scroll_height = available.max(0.0);
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
        },
    );
}
