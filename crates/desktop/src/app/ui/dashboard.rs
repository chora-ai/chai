//! Dashboard-style boxes: grouped frames, key/value rows, two-column layout.
//!
//! Key/value **typography and alignment** for [`kv`] live only here — adjust
//! [`kv_key_rich`] / [`kv_value_rich`] (or spacing in [`super::spacing`]) instead of per-call styling.

use eframe::egui;

use super::spacing;

/// Key label for dashboard rows (muted gray — keys recede, values read as primary).
pub fn kv_key_rich(ui: &egui::Ui, key: &str) -> egui::RichText {
    egui::RichText::new(format!("{}:", key)).color(ui.visuals().weak_text_color())
}

/// Value label for dashboard rows (normal text color so values stand out).
pub fn kv_value_rich(ui: &egui::Ui, value: &str) -> egui::RichText {
    egui::RichText::new(value).color(ui.visuals().text_color())
}

/// Column header in a striped [`egui::Grid`] — same muted color as [`kv_key_rich`] keys (no colon).
pub fn grid_header_rich(ui: &egui::Ui, label: &str) -> egui::RichText {
    egui::RichText::new(label).color(ui.visuals().weak_text_color())
}

/// Grouped frame with a title (Config / Status dashboard columns).
pub fn section_group(ui: &mut egui::Ui, title: &str, contents: impl FnOnce(&mut egui::Ui)) {
    let w = ui.available_width();
    ui.allocate_ui(egui::Vec2::new(w, 0.0), |ui| {
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(spacing::GROUP_INNER_MARGIN))
            .show(ui, |ui| {
                // `Frame::group` shrinks the inner rect by `inner_margin`; use inner width so
                // content does not paint wider than the frame (avoids column overlap).
                ui.set_width(ui.available_width());
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(title).strong());
                    ui.add_space(spacing::GROUP_TITLE_AFTER);
                    ui.separator();
                    ui.add_space(spacing::GROUP_AFTER_SEPARATOR);
                    contents(ui);
                });
            });
    });
}

/// Single key/value row: fixed-width key column (values align vertically), top-aligned so the key
/// and value sit on the same baseline row (avoids centered-vs-top mismatch from nested layouts).
pub fn kv(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.horizontal_top(|ui| {
        let gap = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = spacing::KV_KEY_VALUE_GAP;
        ui.allocate_ui_with_layout(
            egui::vec2(spacing::KV_LABEL_COLUMN_WIDTH, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.label(kv_key_rich(ui, key));
            },
        );
        ui.add(egui::Label::new(kv_value_rich(ui, value)));
        ui.spacing_mut().item_spacing.x = gap;
    });
    ui.add_space(spacing::KV_AFTER);
}

/// Horizontal inset inside one [`egui::Grid`] cell — use for each header or value so content is
/// not flush with the striped row background (egui does not expose per-cell padding on `Grid`).
pub fn grid_cell(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(spacing::GRID_CELL_INNER_PAD_X);
        add_contents(ui);
        ui.add_space(spacing::GRID_CELL_INNER_PAD_X);
    });
}

/// Two equal columns with [`spacing::DASHBOARD_COLUMN_GAP`] between them; restores
/// `item_spacing.x` afterward (matches Skills / Context pattern).
pub fn dashboard_two_columns<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui, &mut egui::Ui) -> R,
) -> R {
    let old_x = ui.spacing().item_spacing.x;
    ui.style_mut().spacing.item_spacing.x = spacing::DASHBOARD_COLUMN_GAP;
    let out = ui.columns(2, |cols| {
        let (left, right) = cols.split_at_mut(1);
        add_contents(&mut left[0], &mut right[0])
    });
    ui.style_mut().spacing.item_spacing.x = old_x;
    out
}
