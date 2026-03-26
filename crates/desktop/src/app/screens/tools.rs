use eframe::egui;

use crate::app::ui::readonly_code;
use crate::app::ChaiApp;

/// Merged OpenAI-style tool definitions from gateway **`status`** (`tools` field).
pub fn ui_tools_screen(app: &mut ChaiApp, ui: &mut egui::Ui, running: bool) {
    crate::app::ui_screen(
        ui,
        "Tools",
        Some(if running {
            "Values below are loaded from the gateway status."
        } else {
            "Start the gateway to load the tool definitions."
        }),
        |ui| {
            if !running {
                app.tools_display_buffer.clear();
                return;
            }

            // Keep showing the last status while a background refetch is in flight (`status_receiver`).
            // Only show loading when we have never received a snapshot yet.
            if app.gateway_status.is_none() {
                ui.label("Loading from gateway status...");
                return;
            }

            let tools_str = app
                .gateway_status
                .as_ref()
                .and_then(|s| s.tools.as_deref())
                .filter(|s| !s.trim().is_empty());

            if let Some(t) = tools_str {
                // Sync only when status content changes. Recreating a new `String` for `TextEdit`
                // every frame resets scroll/layout and causes flicker on periodic status refresh.
                if app.tools_display_buffer.as_str() != t {
                    app.tools_display_buffer = t.to_string();
                }
                readonly_code::read_only_code_scroll(
                    ui,
                    "tools_merged_scroll",
                    "tools_merged_textedit",
                    &mut app.tools_display_buffer,
                    20,
                );
            } else {
                app.tools_display_buffer.clear();
                ui.label(egui::RichText::new("No tools reported in status.").weak());
            }
        },
    );
}
