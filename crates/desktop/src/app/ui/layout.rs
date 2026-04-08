//! Shared layout wrappers for central panel content.

use eframe::egui;

use super::spacing;

/// Central panel body with symmetric horizontal padding (matches prior `Frame::none` + margin).
pub fn central_padded<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(
            spacing::CENTRAL_PANEL_H_MARGIN,
            0.0,
        ))
        .show(ui, add_contents)
}
