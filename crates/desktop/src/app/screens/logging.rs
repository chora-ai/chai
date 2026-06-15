use eframe::egui;

use crate::app::state::logs::log_buffer;
use crate::app::ChaiApp;

/// Extract the timestamp portion of a formatted log line for sorting.
///
/// Log lines have the format `[<timestamp> <LEVEL> <source>] <message>`.
/// The timestamp (between `[` and the first space) is ISO 8601 and
/// lexicographically sortable, so we can sort by it directly.
/// Returns `None` for lines that don't match the expected format
/// (e.g. continuation lines of a multiline log record).
fn log_timestamp(line: &str) -> Option<&str> {
    if let Some(rest) = line.strip_prefix('[') {
        if let Some(end) = rest.find(' ') {
            return Some(&rest[..end]);
        }
    }
    None
}

pub fn ui_logging_screen(_app: &ChaiApp, ui: &mut egui::Ui) {
    let lines: Vec<String> = log_buffer()
        .lock()
        .map(|b| b.iter().cloned().collect())
        .unwrap_or_default();

    // Build sort keys that keep multiline log records grouped together.
    // Walk the lines in arrival order, propagating the last known timestamp
    // forward so that continuation lines (which lack a `[timestamp …]` prefix)
    // inherit the timestamp of their parent header line. Then stable-sort by
    // these keys so gateway logs interleave correctly with desktop logs while
    // multiline records stay together.
    let mut sort_keys: Vec<String> = Vec::with_capacity(lines.len());
    let mut last_ts = String::new();
    for line in &lines {
        if let Some(ts) = log_timestamp(line) {
            last_ts = ts.to_owned();
        }
        sort_keys.push(last_ts.clone());
    }

    // Sort indices by their timestamp key, then reorder lines.
    let mut indices: Vec<usize> = (0..lines.len()).collect();
    indices.sort_by(|&a, &b| sort_keys[a].cmp(&sort_keys[b]));
    let sorted: Vec<String> = indices.iter().map(|&i| lines[i].clone()).collect();

    crate::app::ui_screen(
        ui,
        "Logging",
        Some("Logs loaded from desktop app and connected gateway."),
        |ui| {
            let available = ui.available_height();
            let scroll_height = available.max(0.0);
            egui::ScrollArea::vertical()
                .max_height(scroll_height)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &sorted {
                        ui.label(
                            egui::RichText::new(line.as_str()).family(egui::FontFamily::Monospace),
                        );
                    }
                    if sorted.is_empty() {
                        ui.label("No log output yet.");
                    }
                });
        },
    );
}
