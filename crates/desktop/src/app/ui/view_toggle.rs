//! Dashboard vs raw JSON view toggles (Config / Status screens).

use eframe::egui;

use crate::app::{ConfigViewMode, StatusViewMode};

pub fn config_view_radios(ui: &mut egui::Ui, mode: &mut ConfigViewMode) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("View").strong());
        ui.radio_value(mode, ConfigViewMode::Dashboard, "Dashboard");
        ui.radio_value(mode, ConfigViewMode::RawJson, "Raw JSON");
    });
}

pub fn status_view_radios(ui: &mut egui::Ui, mode: &mut StatusViewMode) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("View").strong());
        ui.radio_value(mode, StatusViewMode::Dashboard, "Dashboard");
        ui.radio_value(mode, StatusViewMode::RawJson, "Raw JSON");
    });
}
