//! Read-only multiline “code” views (JSON, tools) with consistent egui ids.

use eframe::egui;

/// Read-only code editor inside a vertical scroll area (fills available height).
pub fn read_only_code_scroll(
    ui: &mut egui::Ui,
    scroll_id: impl std::hash::Hash,
    textedit_id: impl std::hash::Hash,
    buffer: &mut String,
    min_rows: usize,
) {
    egui::ScrollArea::vertical()
        .id_source(scroll_id)
        .max_height(ui.available_height())
        .show(ui, |ui| {
            egui::TextEdit::multiline(buffer)
                .id(egui::Id::new(textedit_id))
                .code_editor()
                .desired_width(ui.available_width())
                .desired_rows(min_rows)
                .interactive(false)
                .show(ui);
        });
}

/// Read-only code editor when the parent already provides scrolling (e.g. outer `ScrollArea`).
pub fn read_only_code_block(
    ui: &mut egui::Ui,
    textedit_id: impl std::hash::Hash,
    buffer: &mut String,
    min_rows: usize,
) {
    egui::TextEdit::multiline(buffer)
        .id(egui::Id::new(textedit_id))
        .code_editor()
        .desired_width(ui.available_width())
        .desired_rows(min_rows)
        .interactive(false)
        .show(ui);
}
