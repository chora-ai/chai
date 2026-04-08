use eframe::egui;

use crate::app::types::GatewayStatusDetails;
use crate::app::ui::{dashboard, spacing};
use crate::app::ChaiApp;

pub fn ui_context_screen(app: &mut ChaiApp, ui: &mut egui::Ui, running: bool) {
    crate::app::ui_screen(
        ui,
        "Context",
        Some(if running {
            "Static system context per agent from gateway status (orchestrator and each worker)."
        } else {
            "Start the gateway to load the system context."
        }),
        |ui| {
            if !running {
                return;
            }

            let total_height = ui.available_height();
            let Some(ref gs) = app.gateway_status else {
                ui.label("Loading from gateway status...");
                return;
            };

            let orch_id = gs.orchestrator_id.as_deref().unwrap_or("orchestrator");
            let orch_owned = orch_id.to_string();
            let selected_id = app
                .dashboard_agent_id
                .clone()
                .unwrap_or_else(|| orch_owned.clone());
            let is_orchestrator_view =
                gs.agent_system_contexts.is_empty() || selected_id.as_str() == orch_id;

            if gs.agent_system_contexts.len() > 1 {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Agent").strong());
                    egui::ComboBox::from_id_source("context_agent_pick")
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

            let context_text = effective_system_context(gs, selected_id.as_str());

            let is_read_on_demand_orch = if let Some(mode) = gs.context_mode.as_deref() {
                mode == "readOnDemand"
            } else {
                let config = lib::config::load_config(None)
                    .map(|(c, _)| c)
                    .unwrap_or_default();
                matches!(
                    lib::config::orchestrator_context_mode(&config.agents),
                    lib::config::SkillContextMode::ReadOnDemand
                )
            };

            let is_read_on_demand_worker = if !is_orchestrator_view {
                lib::config::load_config(None)
                    .ok()
                    .and_then(|(config, _)| {
                        config.agents.workers.as_ref().and_then(|ws| {
                            ws.iter().find(|w| w.id == selected_id).map(|w| {
                                matches!(
                                    lib::config::worker_context_mode(w),
                                    lib::config::SkillContextMode::ReadOnDemand
                                )
                            })
                        })
                    })
                    .unwrap_or(false)
            } else {
                false
            };

            let use_read_on_demand_two_columns = (is_orchestrator_view && is_read_on_demand_orch)
                || (!is_orchestrator_view && is_read_on_demand_worker);

            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), total_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    if use_read_on_demand_two_columns {
                        dashboard::dashboard_two_columns(ui, |ui_left, ui_right| {
                            if let Some(text) = context_text {
                                let left_title = if is_orchestrator_view {
                                    "System Context"
                                } else {
                                    "Worker system context"
                                };
                                ui_left.label(egui::RichText::new(left_title).strong());
                                ui_left.add_space(spacing::LINE);
                                let mut buf = text.to_string();
                                let scroll_height = ui_left.available_height();
                                let scroll_id = if is_orchestrator_view {
                                    "context_system_scroll"
                                } else {
                                    "context_worker_system_scroll"
                                };
                                egui::ScrollArea::vertical()
                                    .id_source(scroll_id)
                                    .max_height(scroll_height)
                                    .show(ui_left, |ui| {
                                        egui::TextEdit::multiline(&mut buf)
                                            .code_editor()
                                            .desired_width(ui.available_width())
                                            .interactive(false)
                                            .show(ui);
                                    });
                            } else {
                                ui_left.label("No context loaded.");
                            }

                            if let Ok((config, paths)) = lib::config::load_config(None) {
                                let (skill_names, right_title): (&[String], &'static str) =
                                    if is_orchestrator_view {
                                        (
                                            lib::config::orchestrator_skills_enabled_list(
                                                &config.agents,
                                            ),
                                            "Skill bodies (orchestrator)",
                                        )
                                    } else if let Some(w) = config
                                        .agents
                                        .workers
                                        .as_ref()
                                        .and_then(|ws| ws.iter().find(|w| w.id == selected_id))
                                    {
                                        (
                                            lib::config::worker_skills_enabled_list(w),
                                            "Skill bodies (worker)",
                                        )
                                    } else {
                                        (&[], "Skill bodies")
                                    };

                                if !skill_names.is_empty() {
                                    let skills_root =
                                        lib::config::default_skills_dir(&paths.chai_home);
                                    match lib::skills::load_skills(skills_root.as_path()) {
                                        Ok(mut entries) => {
                                            entries.retain(|e| {
                                                skill_names.iter().any(|n| n == &e.name)
                                            });
                                            if entries.is_empty() {
                                                ui_right.label("No enabled skills were loaded.");
                                            } else {
                                                entries.sort_by(|a, b| a.name.cmp(&b.name));
                                                ui_right.label(
                                                    egui::RichText::new(right_title).strong(),
                                                );
                                                ui_right.add_space(spacing::LINE);
                                                let scroll_id = if is_orchestrator_view {
                                                    "context_skills_scroll"
                                                } else {
                                                    "context_worker_skills_scroll"
                                                };
                                                egui::ScrollArea::vertical()
                                                    .id_source(scroll_id)
                                                    .max_height(ui_right.available_height())
                                                    .show(ui_right, |ui| {
                                                        for entry in &entries {
                                                            let body = strip_skill_frontmatter(
                                                                &entry.content,
                                                            );
                                                            let has_body = !body.trim().is_empty();

                                                            ui.label(
                                                                egui::RichText::new(&entry.name)
                                                                    .strong(),
                                                            );
                                                            ui.add_space(spacing::LINE);

                                                            if has_body {
                                                                let mut buf = body.to_string();
                                                                egui::TextEdit::multiline(&mut buf)
                                                                    .code_editor()
                                                                    .desired_width(
                                                                        ui.available_width(),
                                                                    )
                                                                    .interactive(false)
                                                                    .show(ui);
                                                            }

                                                            ui.add_space(spacing::SUBSECTION);
                                                        }
                                                    });
                                            }
                                        }
                                        Err(e) => {
                                            ui_right.colored_label(
                                                egui::Color32::RED,
                                                format!("failed to load skills: {}", e),
                                            );
                                            ui_right.add_space(spacing::LINE);
                                        }
                                    }
                                } else {
                                    ui_right.label(egui::RichText::new(right_title).weak());
                                    ui_right.label("No skills enabled for this agent.");
                                }
                            }
                        });
                    } else {
                        let title = if is_orchestrator_view {
                            "System Context"
                        } else {
                            "Worker system context"
                        };
                        ui.label(egui::RichText::new(title).strong());
                        ui.add_space(spacing::LINE);

                        if let Some(text) = context_text {
                            let mut buf = text.to_string();
                            let scroll_height = ui.available_height();
                            let id = if is_orchestrator_view {
                                "context_system_scroll_full"
                            } else {
                                "context_worker_scroll"
                            };
                            egui::ScrollArea::vertical()
                                .id_source(id)
                                .max_height(scroll_height)
                                .show(ui, |ui| {
                                    egui::TextEdit::multiline(&mut buf)
                                        .code_editor()
                                        .desired_width(ui.available_width())
                                        .interactive(false)
                                        .show(ui);
                                });
                        } else {
                            ui.label("No context loaded.");
                        }
                    }
                },
            );
        },
    );
}

fn effective_system_context<'a>(
    details: &'a GatewayStatusDetails,
    selected_id: &str,
) -> Option<&'a str> {
    if !details.agent_system_contexts.is_empty() {
        return details
            .agent_system_contexts
            .get(selected_id)
            .map(|s| s.as_str());
    }
    details.system_context.as_deref()
}

/// Strip YAML frontmatter (`---` ... `---`) from SKILL.md content so that the
/// visible body matches what the gateway sends via read_skill and in
/// skills_context_bodies.
fn strip_skill_frontmatter(content: &str) -> &str {
    let rest = content.trim_start();
    let rest = rest
        .strip_prefix("---")
        .map(|s| s.trim_start())
        .unwrap_or(rest);
    if let Some(i) = rest.find("\n---") {
        let after = rest
            .get(i + 4..)
            .unwrap_or_else(|| &rest[rest.len()..])
            .trim_start();
        if after.starts_with("---") {
            return strip_skill_frontmatter(after);
        }
        after
    } else if rest == "---" {
        &rest[rest.len()..]
    } else {
        rest
    }
}
