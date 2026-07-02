use eframe::egui;

use crate::app::ui::{readonly_code, spacing};
use crate::app::ChaiApp;

pub fn ui_tools_screen(app: &mut ChaiApp, ui: &mut egui::Ui, running: bool) {
    crate::app::ui_screen(
        ui,
        "Tools",
        Some(if running {
            "Sent as tool schemas on every turn (built at startup, separate from messages)."
        } else {
            "Start the gateway to load agent tools."
        }),
        |ui| {
            if !running {
                app.tools_display_buffer.clear();
                return;
            }

            // Clone the needed gateway status data so we don't hold an immutable borrow of `app`.
            let gs = match app.gateway_status() {
                Some(g) => g.clone(),
                None => {
                    ui.label("Loading from gateway status...");
                    return;
                }
            };
            let orch_ids: std::collections::HashSet<&str> = gs
                .orchestrators
                .iter()
                .map(|o| o.id.as_str())
                .collect();
            let orch_id = gs.orchestrator_id().unwrap_or("orchestrator");
            let orch_owned = orch_id.to_string();
            let selected_id = app
                .dashboard_agent_id()
                .cloned()
                .unwrap_or_else(|| orch_owned.clone());

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Agent").strong());
                egui::ComboBox::from_id_source("tools_agent_pick")
                    .selected_text(&selected_id)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for id in gs.agent_skills.keys() {
                            let suffix = if orch_ids.contains(id.as_str()) {
                                " — orchestrator"
                            } else {
                                " — worker"
                            };
                            let label = format!("{}{}", id, suffix);
                            if ui
                                .selectable_label(selected_id == id.as_str(), label)
                                .clicked()
                            {
                                *app.dashboard_agent_id_mut() = Some(id.clone());
                            }
                        }
                    });
            });
            ui.add_space(spacing::SUBSECTION);

            // Get tools from on-demand agent detail cache.
            let tools_str = app
                .agent_detail_cache()
                .and_then(|c| c.get(&selected_id))
                .and_then(|d| d.tools.as_deref());

            if let Some(t) = tools_str {
                if app.tools_display_buffer.as_str() != t {
                    app.tools_display_buffer = t.to_string();
                }
                let scroll_id = format!("tools_merged_scroll_{}", selected_id);
                let text_id = format!("tools_merged_textedit_{}", selected_id);
                readonly_code::read_only_code_scroll(
                    ui,
                    &scroll_id,
                    &text_id,
                    &mut app.tools_display_buffer,
                    20,
                );
            } else if app.agent_detail_cache().map_or(false, |c| c.contains_key(&selected_id)) {
                // Detail is loaded but this agent has no tools.
                app.tools_display_buffer.clear();
                ui.label(egui::RichText::new("No tools reported for this agent.").weak());
            } else {
                // Agent detail not yet loaded — show error or loading state.
                if let Some((ref err_id, ref err_msg)) = app.agent_detail_fetch_error() {
                    if err_id == &selected_id {
                        ui.colored_label(egui::Color32::RED, err_msg);
                    } else {
                        ui.label("Loading agent detail...");
                    }
                } else {
                    ui.label("Loading agent detail...");
                }
            }
        },
    );
}
