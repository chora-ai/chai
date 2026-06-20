use eframe::egui;

use crate::app::ui::{dashboard, spacing};
use crate::app::ChaiApp;

pub fn ui_agent_screen(app: &mut ChaiApp, ui: &mut egui::Ui, running: bool) {
    crate::app::ui_screen(
        ui,
        "Agent",
        Some(if running {
            "Injected as a system message on every turn (built at startup, separate from history)."
        } else {
            "Start the gateway to load agent context."
        }),
        |ui| {
            if !running {
                return;
            }

            let total_height = ui.available_height();

            // Load cached config before borrowing gateway status, to avoid a
            // simultaneous &self.gateway_status and &mut self.cached_config.
            let cached_config = app.load_config_cached()
                .map(|(c, _)| c.clone())
                .unwrap_or_default();

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
                gs.agent_skills.is_empty() || selected_id.as_str() == orch_id;

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Agent").strong());
                egui::ComboBox::from_id_source("context_agent_pick")
                    .selected_text(&selected_id)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for id in gs.agent_skills.keys() {
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

            // Resolve context mode: prefer gateway status, fall back to cached config.
            let is_read_on_demand_orch = if let Some(rt) = gs.agent_skills.get(&selected_id) {
                rt.context_mode.as_deref() == Some("readOnDemand")
            } else {
                matches!(
                    lib::config::orchestrator_context_mode(&cached_config.agents),
                    lib::config::SkillContextMode::ReadOnDemand
                )
            };

            let is_read_on_demand_worker = if !is_orchestrator_view {
                // Prefer per-agent context mode from gateway status.
                if let Some(mode) = gs
                    .agent_skills
                    .get(&selected_id)
                    .and_then(|rt| rt.context_mode.as_deref())
                {
                    mode == "readOnDemand"
                } else {
                    cached_config.agents.workers.as_ref().and_then(|ws| {
                        ws.iter().find(|w| w.id == selected_id).map(|w| {
                            matches!(
                                lib::config::worker_context_mode(w),
                                lib::config::SkillContextMode::ReadOnDemand
                            )
                        })
                    })
                    .unwrap_or(false)
                }
            } else {
                false
            };

            let use_read_on_demand_two_columns = (is_orchestrator_view && is_read_on_demand_orch)
                || (!is_orchestrator_view && is_read_on_demand_worker);

            // Get on-demand agent detail (system context + skills context).
            let agent_detail = app.agent_detail_cache.get(&selected_id).cloned();
            let context_text = agent_detail.as_ref().and_then(|d| d.system_context.as_deref());
            let status_bodies = agent_detail.as_ref().map(|d| d.skills_context.clone()).filter(|m| !m.is_empty());

            // If agent detail is not yet loaded, show loading state.
            if agent_detail.is_none() {
                ui.label("Loading agent detail...");
                return;
            }

            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), total_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    if use_read_on_demand_two_columns {
                        dashboard::dashboard_two_columns(ui, |ui_left, ui_right| {
                            if let Some(text) = context_text {
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

                            if let Some(bodies_map) = &status_bodies {
                                let scroll_id = if is_orchestrator_view {
                                    "context_skills_scroll"
                                } else {
                                    "context_worker_skills_scroll"
                                };
                                egui::ScrollArea::vertical()
                                    .id_source(scroll_id)
                                    .max_height(ui_right.available_height())
                                    .show(ui_right, |ui| {
                                        for (skill_name, skill_body) in bodies_map {
                                            let has_body = !skill_body.trim().is_empty();
                                            egui::Frame::group(ui.style())
                                                .inner_margin(egui::Margin::same(
                                                    spacing::GROUP_INNER_MARGIN,
                                                ))
                                                .show(ui, |ui| {
                                                    ui.set_width(ui.available_width());
                                                    ui.vertical(|ui| {
                                                        ui.label(
                                                            egui::RichText::new(skill_name)
                                                                .strong(),
                                                        );
                                                        ui.add_space(spacing::GROUP_TITLE_AFTER);
                                                        let sep_stroke = egui::Stroke::new(
                                                            1.0,
                                                            ui.visuals()
                                                                .widgets
                                                                .noninteractive
                                                                .bg_stroke
                                                                .color,
                                                        );
                                                        let sep_w = ui.available_width();
                                                        let (sep_rect, _) = ui
                                                            .allocate_exact_size(
                                                                egui::vec2(sep_w, 1.0),
                                                                egui::Sense::hover(),
                                                            );
                                                        ui.painter().hline(
                                                            sep_rect.x_range(),
                                                            sep_rect.center().y,
                                                            sep_stroke,
                                                        );
                                                        ui.add_space(
                                                            spacing::GROUP_AFTER_SEPARATOR,
                                                        );

                                                        if has_body {
                                                            let mut buf =
                                                                skill_body.to_string();
                                                            egui::TextEdit::multiline(&mut buf)
                                                                .code_editor()
                                                                .desired_width(
                                                                    ui.available_width(),
                                                                )
                                                                .interactive(false)
                                                                .show(ui);
                                                        } else {
                                                            ui.label("No SKILL.md content.");
                                                        }
                                                    });
                                                });
                                            ui.add_space(spacing::SUBSECTION);
                                        }
                                    });
                            } else {
                                // No on-demand skill data — fall back to cached config.
                                {
                                    let config = &cached_config;
                                    let (skill_names, right_title): (&[String], &'static str) =
                                        if is_orchestrator_view {
                                            (
                                                lib::config::orchestrator_enabled_skills_list(
                                                    &config.agents,
                                                ),
                                                "Skill bodies (orchestrator)",
                                            )
                                        } else if let Some(w) = config
                                            .agents
                                            .workers
                                            .as_ref()
                                            .and_then(|ws| {
                                                ws.iter().find(|w| w.id == selected_id)
                                            })
                                        {
                                            (
                                                lib::config::worker_enabled_skills_list(w),
                                                "Skill bodies (worker)",
                                            )
                                        } else {
                                            (&[], "Skill bodies")
                                        };

                                    if !skill_names.is_empty() {
                                        if let Some(ref cached) = app.cached_skills {
                                            let mut entries: Vec<_> = cached
                                                .iter()
                                                .filter(|e| {
                                                    skill_names.iter().any(|n| n == &e.name)
                                                })
                                                .cloned()
                                                .collect();
                                            if entries.is_empty() {
                                                ui_right
                                                    .label("No enabled skills were loaded.");
                                            } else {
                                                entries.sort_by(|a, b| a.name.cmp(&b.name));
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
                                                            let has_body =
                                                                !body.trim().is_empty();
                                                            egui::Frame::group(ui.style())
                                                                .inner_margin(egui::Margin::same(
                                                                    spacing::GROUP_INNER_MARGIN,
                                                                ))
                                                                .show(ui, |ui| {
                                                                    ui.set_width(
                                                                        ui.available_width(),
                                                                    );
                                                                    ui.vertical(|ui| {
                                                                        ui.label(
                                                                            egui::RichText::new(
                                                                                &entry.name,
                                                                            )
                                                                            .strong(),
                                                                        );
                                                                        ui.add_space(
                                                                            spacing::GROUP_TITLE_AFTER,
                                                                        );
                                                                        let sep_stroke =
                                                                            egui::Stroke::new(
                                                                                1.0,
                                                                                ui.visuals()
                                                                                    .widgets
                                                                                    .noninteractive
                                                                                    .bg_stroke
                                                                                    .color,
                                                                            );
                                                                        let sep_w =
                                                                            ui.available_width();
                                                                        let (sep_rect, _) = ui
                                                                            .allocate_exact_size(
                                                                                egui::vec2(
                                                                                    sep_w,
                                                                                    1.0,
                                                                                ),
                                                                                egui::Sense::hover(),
                                                                            );
                                                                        ui.painter().hline(
                                                                            sep_rect.x_range(),
                                                                            sep_rect.center().y,
                                                                            sep_stroke,
                                                                        );
                                                                        ui.add_space(
                                                                            spacing::GROUP_AFTER_SEPARATOR,
                                                                        );

                                                                        if has_body {
                                                                            let mut buf =
                                                                                body.to_string();
                                                                            egui::TextEdit::multiline(&mut buf)
                                                                                .code_editor()
                                                                                .desired_width(
                                                                                    ui.available_width(),
                                                                                )
                                                                                .interactive(false)
                                                                                .show(ui);
                                                                        } else {
                                                                            ui.label("No SKILL.md content.");
                                                                        }
                                                                    });
                                                                });
                                                            ui.add_space(spacing::SUBSECTION);
                                                        }
                                                    });
                                            }
                                        } else {
                                            ui_right.label("Loading skills...");
                                        }
                                    } else {
                                        ui_right.label(egui::RichText::new(right_title).weak());
                                        ui_right.label("No enabled skills for this agent.");
                                    }
                                }
                            }
                        });
                    } else {
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

/// Strip YAML frontmatter (`---` ... `---`) from SKILL.md content so that the
/// visible body matches what the gateway sends via read_skill and in
/// skillsContext.
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
