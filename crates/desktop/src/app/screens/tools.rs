use eframe::egui;

use crate::app::types::GatewayStatusDetails;
use crate::app::ui::{readonly_code, spacing};
use crate::app::ChaiApp;

fn effective_tools_json<'a>(
    gs: &'a GatewayStatusDetails,
    selected_id: &str,
    orchestrator_id: &str,
) -> Option<&'a str> {
    gs.agent_tools
        .get(selected_id)
        .map(|s| s.as_str())
        .or_else(|| {
            if selected_id == orchestrator_id {
                gs.tools.as_deref()
            } else {
                None
            }
        })
}

/// OpenAI-style tool definitions from gateway **`status`** (**`agentTools`**, with legacy fallback to **`tools`** for the orchestrator).
pub fn ui_tools_screen(app: &mut ChaiApp, ui: &mut egui::Ui, running: bool) {
    crate::app::ui_screen(
        ui,
        "Tools",
        Some(if running {
            "Per-agent tool definitions from gateway status (same list each role receives on a turn)."
        } else {
            "Start the gateway to load the tool definitions."
        }),
        |ui| {
            if !running {
                app.tools_display_buffer.clear();
                return;
            }

            if app.gateway_status.is_none() {
                ui.label("Loading from gateway status...");
                return;
            }

            let gs = app.gateway_status.as_ref().unwrap();
            let orch_id = gs.orchestrator_id.as_deref().unwrap_or("orchestrator");
            let orch_owned = orch_id.to_string();
            let selected_id = app
                .dashboard_agent_id
                .clone()
                .unwrap_or_else(|| orch_owned.clone());

            if gs.agent_system_contexts.len() > 1 {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Agent").strong());
                    egui::ComboBox::from_id_source("tools_agent_pick")
                        .selected_text(&selected_id)
                        .width(220.0)
                        .show_ui(ui, |ui| {
                            for id in gs.agent_system_contexts.keys() {
                                let suffix = if id == orch_id {
                                    " — orchestrator"
                                } else {
                                    " — worker"
                                };
                                let label = format!("{}{}", id, suffix);
                                if ui
                                    .selectable_label(selected_id == id.as_str(), label)
                                    .clicked()
                                {
                                    app.dashboard_agent_id = Some(id.clone());
                                }
                            }
                        });
                });
                ui.add_space(spacing::SUBSECTION);
            }

            let tools_str = effective_tools_json(gs, selected_id.as_str(), orch_id);

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
            } else {
                app.tools_display_buffer.clear();
                ui.label(egui::RichText::new("No tools reported for this agent.").weak());
            }
        },
    );
}
