use eframe::egui;

use crate::app::ui::{dashboard, spacing};
use crate::app::ChaiApp;

fn orchestrator_id_from_config(agents: &lib::config::AgentsConfig) -> String {
    agents
        .orchestrator_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("orchestrator")
        .to_string()
}

fn config_agent_ids(agents: &lib::config::AgentsConfig) -> Vec<String> {
    let mut ids = vec![orchestrator_id_from_config(agents)];
    if let Some(ws) = &agents.workers {
        for w in ws {
            let id = w.id.trim();
            if !id.is_empty() && !ids.iter().any(|x| x == id) {
                ids.push(id.to_string());
            }
        }
    }
    ids
}

/// Keep **Skills** agent selection consistent with **`config.json`** when the gateway is down or has not yet populated status.
fn reconcile_skills_dashboard_agent(app: &mut ChaiApp, agents: &lib::config::AgentsConfig) {
    let ids = config_agent_ids(agents);
    let orch = orchestrator_id_from_config(agents);
    let valid = app
        .dashboard_agent_id
        .as_ref()
        .map(|id| ids.iter().any(|x| x == id))
        .unwrap_or(false);
    if !valid {
        app.dashboard_agent_id = Some(orch);
    }
}

fn skills_enabled_for_agent<'a>(
    agents: &'a lib::config::AgentsConfig,
    agent_id: &str,
    orchestrator_id: &str,
) -> &'a [String] {
    if agent_id == orchestrator_id {
        lib::config::orchestrator_skills_enabled_list(agents)
    } else if let Some(ws) = agents.workers.as_ref() {
        ws.iter()
            .find(|w| w.id.trim() == agent_id)
            .map(lib::config::worker_skills_enabled_list)
            .unwrap_or(&[])
    } else {
        &[]
    }
}

pub fn ui_skills_screen(app: &mut ChaiApp, ui: &mut egui::Ui) {
    let Ok((config, paths)) = lib::config::load_config(None) else {
        crate::app::ui_screen(ui, "Skills", None, |ui| {
            ui.label(egui::RichText::new("could not load profile (run `chai init`)").weak());
        });
        return;
    };

    reconcile_skills_dashboard_agent(app, &config.agents);

    let skills_root = lib::config::default_skills_dir(&paths.chai_home);
    let orch_id = orchestrator_id_from_config(&config.agents);
    let agent_ids = config_agent_ids(&config.agents);
    let selected_id = app
        .dashboard_agent_id
        .as_deref()
        .unwrap_or(orch_id.as_str())
        .to_string();
    let enabled: Vec<String> = skills_enabled_for_agent(&config.agents, &selected_id, &orch_id)
        .iter()
        .cloned()
        .collect();

    let skills_result = lib::skills::load_skills(skills_root.as_path());
    let mut skills = match skills_result {
        Ok(list) => list,
        Err(e) => {
            let subtitle = format!("Values below are loaded from {}", skills_root.display());
            crate::app::ui_screen(ui, "Skills", Some(&subtitle), |ui| {
                ui.colored_label(egui::Color32::RED, format!("failed to load skills: {}", e));
            });
            return;
        }
    };

    if skills.is_empty() {
        let subtitle = format!("Values below are loaded from {}", skills_root.display());
        crate::app::ui_screen(ui, "Skills", Some(&subtitle), |ui| {
            ui.label("No skills found in the configured directories.");
        });
        return;
    }

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    let enabled_set: std::collections::HashSet<String> =
        enabled.into_iter().map(|s| s.trim().to_string()).collect();
    let enabled_skills: Vec<_> = skills
        .iter()
        .filter(|e| enabled_set.contains(e.name.as_str()))
        .collect();
    let disabled_skills: Vec<_> = skills
        .iter()
        .filter(|e| !enabled_set.contains(e.name.as_str()))
        .collect();

    let subtitle = format!(
        "Packages from {}; enabled/disabled for agent \"{}\" (from config).",
        skills_root.display(),
        selected_id
    );

    crate::app::ui_screen(ui, "Skills", Some(&subtitle), |ui| {
        if agent_ids.len() > 1 {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Agent").strong());
                egui::ComboBox::from_id_source("skills_agent_pick")
                    .selected_text(&selected_id)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for id in &agent_ids {
                            let suffix = if id == &orch_id {
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

        dashboard::dashboard_two_columns(ui, |ui_left, ui_right| {
            // Left column: skills list
            {
                egui::ScrollArea::vertical()
                    .id_source(format!("skills_list_scroll_{}", selected_id))
                    .show(ui_left, |ui| {
                        // Enabled section
                        ui.label(egui::RichText::new("Enabled").strong());
                        ui.add_space(spacing::LINE);
                        if enabled_skills.is_empty() {
                            ui.label("No skills enabled.");
                        } else {
                            for entry in enabled_skills.iter() {
                                let title = entry.name.as_str();
                                paint_one_skill(ui, app, entry, title);
                                ui.add_space(spacing::SUBSECTION);
                            }
                        }
                        ui.add_space(spacing::SUBSECTION);

                        // Disabled section
                        ui.label(egui::RichText::new("Disabled").strong());
                        ui.add_space(spacing::LINE);
                        if disabled_skills.is_empty() {
                            ui.label("No skills disabled.");
                        } else {
                            for entry in disabled_skills.iter() {
                                let title = entry.name.as_str();
                                paint_one_skill(ui, app, entry, title);
                                ui.add_space(spacing::SUBSECTION);
                            }
                        }
                    });
            }

            // Right column: SKILL.md (top) + tools.json (bottom) for selected skill
            {
                let selected = app
                    .selected_skill_name
                    .as_ref()
                    .and_then(|name| skills.iter().find(|e| &e.name == name));

                if let Some(entry) = selected {
                    let total_height = ui_right.available_height();
                    let half_height = total_height / 2.0;

                    // Top half: SKILL.md body
                    ui_right.allocate_ui_with_layout(
                        egui::vec2(ui_right.available_width(), half_height),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.label(egui::RichText::new("Skill").strong());
                            ui.add_space(spacing::LINE);

                            let body = strip_skill_frontmatter(&entry.content);
                            if body.trim().is_empty() {
                                ui.label("No SKILL.md content.");
                            } else {
                                let mut buf = body.to_string();
                                egui::ScrollArea::vertical()
                                    .id_source("skills_skillmd_scroll")
                                    .max_height(ui.available_height())
                                    .show(ui, |ui| {
                                        egui::TextEdit::multiline(&mut buf)
                                            .code_editor()
                                            .desired_width(ui.available_width())
                                            .interactive(false)
                                            .show(ui);
                                    });
                            }
                        },
                    );

                    ui_right.add_space(spacing::SUBSECTION);

                    // Bottom half: tools.json (use remaining height after SKILL.md + spacing)
                    let tools_path = entry.path.join("tools.json");
                    match std::fs::read_to_string(&tools_path) {
                        Ok(contents) => {
                            // Pretty-print tools.json with two-space indentation when possible.
                            let pretty = serde_json::from_str::<serde_json::Value>(&contents)
                                .ok()
                                .and_then(|value| serde_json::to_string_pretty(&value).ok())
                                .unwrap_or(contents);

                            ui_right.label(egui::RichText::new("Tools").strong());
                            ui_right.add_space(spacing::LINE);

                            let remaining_height = ui_right.available_height();
                            ui_right.allocate_ui_with_layout(
                                egui::vec2(ui_right.available_width(), remaining_height),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    egui::ScrollArea::vertical()
                                        .id_source("skills_tools_scroll")
                                        .max_height(ui.available_height())
                                        .show(ui, |ui| {
                                            let mut buf = pretty.clone();
                                            egui::TextEdit::multiline(&mut buf)
                                                .code_editor()
                                                .desired_width(ui.available_width())
                                                .interactive(false)
                                                .show(ui);
                                        });
                                },
                            );
                        }
                        Err(_) => {
                            ui_right.label(egui::RichText::new("Tools").strong());
                            ui_right.add_space(spacing::LINE);
                            ui_right.label("No tools.json found for this skill.");
                        }
                    }
                }
            }
        });
    });
}

/// Strip YAML frontmatter (`---` ... `---`) from SKILL.md content so that only
/// the visible body is shown.
fn strip_skill_frontmatter(content: &str) -> &str {
    let rest = content.trim_start();
    let rest = rest
        .strip_prefix("---")
        .map(|s| s.trim_start())
        .unwrap_or(rest);
    if let Some(i) = rest.find("\n---") {
        // Use empty slice when i+4 is out of bounds so we don't return the
        // closing delimiter as body.
        let after = rest
            .get(i + 4..)
            .unwrap_or_else(|| &rest[rest.len()..])
            .trim_start();
        if after.starts_with("---") {
            return strip_skill_frontmatter(after);
        }
        after
    } else if rest == "---" {
        // Frontmatter was "---\n---" with no body; don't return the closing delimiter.
        &rest[rest.len()..]
    } else {
        rest
    }
}

fn skill_field_block(ui: &mut egui::Ui, key: &str, value: &str, selected: bool) {
    let key_rt = if selected {
        egui::RichText::new(format!("{}:", key)).color(egui::Color32::WHITE)
    } else {
        dashboard::kv_key_rich(ui, key)
    };
    let value_rt = if selected {
        egui::RichText::new(value).color(egui::Color32::WHITE)
    } else {
        dashboard::kv_value_rich(ui, value)
    };

    ui.vertical(|ui| {
        ui.label(key_rt);
        ui.add_space(2.0);
        ui.add(egui::Label::new(value_rt).wrap(true));
    });
    ui.add_space(spacing::KV_AFTER);
}

fn paint_one_skill(
    ui: &mut egui::Ui,
    app: &mut ChaiApp,
    entry: &lib::skills::SkillEntry,
    title: &str,
) {
    let selected = app
        .selected_skill_name
        .as_deref()
        .map(|n| n == entry.name.as_str())
        .unwrap_or(false);

    let bg_fill = if selected {
        ui.visuals().selection.bg_fill
    } else {
        egui::Color32::TRANSPARENT
    };
    let stroke = if selected {
        ui.visuals().selection.stroke
    } else {
        egui::Stroke {
            width: 1.0,
            color: ui.visuals().widgets.noninteractive.bg_stroke.color,
        }
    };

    let frame = egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(spacing::GROUP_INNER_MARGIN))
        .fill(bg_fill)
        .stroke(stroke);

    let frame_response = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical(|ui| {
            let title_rt = if selected {
                egui::RichText::new(title)
                    .strong()
                    .color(egui::Color32::WHITE)
            } else {
                egui::RichText::new(title).strong()
            };
            ui.label(title_rt);
            ui.add_space(spacing::GROUP_TITLE_AFTER);

            let sep_stroke = if selected {
                egui::Stroke::new(1.0, egui::Color32::WHITE)
            } else {
                egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
            };
            let sep_w = ui.available_width();
            let (sep_rect, _) =
                ui.allocate_exact_size(egui::vec2(sep_w, 1.0), egui::Sense::hover());
            ui.painter()
                .hline(sep_rect.x_range(), sep_rect.center().y, sep_stroke);
            ui.add_space(spacing::GROUP_AFTER_SEPARATOR);

            if !entry.description.is_empty() {
                let desc = entry.description.trim();
                skill_field_block(ui, "Description", desc, selected);
            }

            let path_str = entry.path.display().to_string();
            skill_field_block(ui, "Path", &path_str, selected);
        });
    });

    let click = ui.interact(
        frame_response.response.rect,
        frame_response.response.id,
        egui::Sense::click(),
    );
    if click.clicked() {
        app.selected_skill_name = Some(entry.name.clone());
    }
}
