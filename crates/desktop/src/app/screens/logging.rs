use std::collections::VecDeque;
use std::sync::Mutex;

use eframe::egui;

use crate::app::state::logs::{desktop_log_buffer, gateway_log_buffer};
use crate::app::ChaiApp;

/// Height of the horizontal separator between the two log panels.
const SEPARATOR_HEIGHT: f32 = 24.0;

/// Render a single log panel with a heading and a stick-to-bottom scroll area.
///
/// `id_source` must be unique per panel so that egui can distinguish the scroll
/// areas and track their scroll offsets independently.
fn log_panel(ui: &mut egui::Ui, heading: &str, buffer: &Mutex<VecDeque<String>>, id_source: &str) {
    ui.heading(egui::RichText::new(heading).size(14.0));
    ui.add_space(8.0);

    let lines: Vec<String> = buffer
        .lock()
        .map(|b| b.iter().cloned().collect())
        .unwrap_or_default();

    // Use the full remaining vertical space in this child UI for the scroll
    // area. Because the parent allocated a fixed-height rect via
    // allocate_exact_size + child_ui, available_height() is the exact
    // remaining space after the heading — the panel will not grow with content.
    let scroll_height = ui.available_height().max(0.0);

    egui::ScrollArea::vertical()
        .id_source(id_source)
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

    ui.add_space(8.0);
}

pub fn ui_logging_screen(_app: &ChaiApp, ui: &mut egui::Ui) {
    crate::app::ui_screen(
        ui,
        "Logging",
        Some("Logs loaded from desktop app and connected gateway."),
        |ui| {
            let available = ui.available_height();
            // 30.0 accounts for header and padding included in log_panel
            let half_height = ((available - SEPARATOR_HEIGHT - 30.0) / 2.0).max(0.0);
            let width = ui.available_width();

            // Desktop logs — top half.
            // Use allocate_exact_size + child_ui (same pattern as the Chat screen)
            // so the panel gets a truly fixed rect that does not grow with content.
            let top_rect = ui
                .allocate_exact_size(egui::vec2(width, half_height), egui::Sense::hover())
                .0;
            let mut top_ui = ui.child_ui(top_rect, egui::Layout::top_down(egui::Align::Min));
            log_panel(&mut top_ui, "Desktop Logs", desktop_log_buffer(), "desktop_logs_scroll");

            // Horizontal divider.
            let sep_stroke =
                egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
            let (sep_rect, _) =
                ui.allocate_exact_size(egui::vec2(width, SEPARATOR_HEIGHT), egui::Sense::hover());
            ui.painter()
                .hline(sep_rect.x_range(), sep_rect.center().y, sep_stroke);

            // Gateway logs — bottom half.
            let bottom_rect = ui
                .allocate_exact_size(egui::vec2(width, half_height), egui::Sense::hover())
                .0;
            let mut bottom_ui = ui.child_ui(bottom_rect, egui::Layout::top_down(egui::Align::Min));
            log_panel(&mut bottom_ui, "Gateway Logs", gateway_log_buffer(), "gateway_logs_scroll");
        },
    );
}
