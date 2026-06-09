use eframe::egui;

use crate::app::ChaiApp;

pub fn ui_files_screen(_app: &mut ChaiApp, ui: &mut egui::Ui, _running: bool) {
    crate::app::ui_screen(
        ui,
        "Files",
        Some("File explorer not yet implemented."),
        |_| {},
    );
}
