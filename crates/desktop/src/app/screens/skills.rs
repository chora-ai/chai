use eframe::egui;

use crate::app::ui::{dashboard, spacing};
use crate::app::ChaiApp;

pub fn ui_skills_screen(app: &mut ChaiApp, ui: &mut egui::Ui) {
    let (config, chai_home) = match app.load_config_cached() {
        Ok(cp) => (cp.0.clone(), cp.1.chai_home.clone()),
        Err(e) => {
            crate::app::ui_screen(ui, "Skills", Some("Fix the error below to load skills."), |ui| {
                ui.colored_label(egui::Color32::RED, format!("failed to load config: {}", e));
            });
            return;
        }
    };

    let skills_root = lib::config::default_skills_dir(&chai_home);

    let Some(ref cached) = app.cached_skills else {
        let subtitle = format!("Values below are from {}", skills_root.display());
        crate::app::ui_screen(ui, "Skills", Some(&subtitle), |ui| {
            if let Some(ref err) = app.skills_fetch_error {
                ui.colored_label(egui::Color32::RED, err);
            } else {
                ui.label("Loading skills...");
            }
        });
        return;
    };

    let mut skills = cached.clone();

    if skills.is_empty() {
        let subtitle = format!("Values below are from {}", skills_root.display());
        crate::app::ui_screen(ui, "Skills", Some(&subtitle), |ui| {
            ui.label(format!("No skills found in {}.", skills_root.display()));
        });
        return;
    }

    skills.sort_by(|a, b| a.name.cmp(&b.name));

    let skill_agent_map = build_skill_agent_map(&config, app.gateway_status.as_ref());

    let subtitle = format!(
        "Skill packages loaded from {}.",
        skills_root.display()
    );

    crate::app::ui_screen(ui, "Skills", Some(&subtitle), |ui| {
        dashboard::dashboard_two_columns(ui, |ui_left, ui_right| {
            // Left column: skills list (all skills in alphabetical order)
            {
                egui::ScrollArea::vertical()
                    .id_source("skills_list_scroll")
                    .show(ui_left, |ui| {
                        for entry in skills.iter() {
                            let agents_for_skill = skill_agent_map.get(&entry.name);
                            paint_one_skill(ui, app, entry, agents_for_skill);
                            ui.add_space(spacing::SUBSECTION);
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

fn orchestrator_id_from_config(agents: &lib::config::AgentsConfig) -> String {
    agents
        .orchestrator_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("orchestrator")
        .to_string()
}

fn enabled_skills_for_agent<'a>(
    agents: &'a lib::config::AgentsConfig,
    agent_id: &str,
    orchestrator_id: &str,
) -> &'a [String] {
    if agent_id == orchestrator_id {
        lib::config::orchestrator_enabled_skills_list(agents)
    } else if let Some(ws) = agents.workers.as_ref() {
        ws.iter()
            .find(|w| w.id.trim() == agent_id)
            .map(lib::config::worker_enabled_skills_list)
            .unwrap_or(&[])
    } else {
        &[]
    }
}

/// Build a map from skill name → list of (agent_id, is_orchestrator) for all
/// agents that have the skill enabled. Uses gateway status as the source of
/// truth when available; falls back to config only when there is no gateway
/// connection.
fn build_skill_agent_map(
    config: &lib::config::Config,
    gateway_status: Option<&crate::app::types::GatewayStatusDetails>,
) -> std::collections::BTreeMap<String, Vec<(String, bool)>> {
    let mut map: std::collections::BTreeMap<String, Vec<(String, bool)>> =
        std::collections::BTreeMap::new();

    if let Some(gs) = gateway_status {
        // Gateway is connected: iterate its agent_skills directly. The
        // gateway is the source of truth for which agents are running and
        // which skills each has enabled — even if the config on disk has
        // been edited but the gateway has not been restarted.
        let orch_id = gs
            .orchestrator_id
            .as_deref()
            .unwrap_or("orchestrator");
        for (agent_id, rt) in &gs.agent_skills {
            let is_orchestrator = agent_id == orch_id;
            for name in &rt.enabled_skills {
                map.entry(name.trim().to_string())
                    .or_default()
                    .push((agent_id.clone(), is_orchestrator));
            }
        }
    } else {
        // No gateway connection: fall back to config so the screen still
        // shows something useful.
        let orch_id = orchestrator_id_from_config(&config.agents);
        for name in enabled_skills_for_agent(&config.agents, &orch_id, &orch_id)
            .iter()
            .map(|s| s.trim().to_string())
        {
            map.entry(name).or_default().push((orch_id.clone(), true));
        }
        if let Some(ws) = &config.agents.workers {
            for w in ws {
                let worker_id = w.id.trim().to_string();
                if worker_id.is_empty() || worker_id == orch_id {
                    continue;
                }
                for name in enabled_skills_for_agent(&config.agents, &worker_id, &orch_id)
                    .iter()
                    .map(|s| s.trim().to_string())
                {
                    map.entry(name).or_default().push((worker_id.clone(), false));
                }
            }
        }
    }

    map
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

fn skill_field_block(ui: &mut egui::Ui, key: &str, value: &str, _selected: bool) {
    let key_rt = dashboard::kv_key_rich(ui, key);
    let value_rt = dashboard::kv_value_rich(ui, value);

    ui.vertical(|ui| {
        ui.label(key_rt);
        ui.add_space(2.0);
        ui.add(egui::Label::new(value_rt).wrap(true));
    });
}

fn paint_one_skill(
    ui: &mut egui::Ui,
    app: &mut ChaiApp,
    entry: &lib::skills::SkillEntry,
    agents_for_skill: Option<&Vec<(String, bool)>>,
) {
    let selected = app
        .selected_skill_name
        .as_deref()
        .map(|n| n == entry.name.as_str())
        .unwrap_or(false);

    let bg_fill = egui::Color32::TRANSPARENT;
    let stroke = if selected {
        egui::Stroke {
            width: 1.0,
            color: ui.visuals().widgets.active.bg_stroke.color,
        }
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
            let title_rt = egui::RichText::new(&entry.name).strong();
            ui.label(title_rt);
            ui.add_space(spacing::GROUP_TITLE_AFTER);
            let sep_stroke = egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
            let sep_w = ui.available_width();
            let (sep_rect, _) =
                ui.allocate_exact_size(egui::vec2(sep_w, 1.0), egui::Sense::hover());
            ui.painter()
                .hline(sep_rect.x_range(), sep_rect.center().y, sep_stroke);
            ui.add_space(spacing::GROUP_AFTER_SEPARATOR);
            ui.add_space(8.0);

            // Show enabled-for agents
            if let Some(agents) = agents_for_skill {
                for (agent_id, is_orchestrator) in agents {
                    let label = format!("Enabled for {}", agent_id);
                    let rt = if *is_orchestrator {
                        egui::RichText::new(label).color(egui::Color32::from_rgb(120, 150, 120))
                    } else {
                        egui::RichText::new(label).color(egui::Color32::from_rgb(120, 120, 150))
                    };
                    ui.label(rt);
                }
            } else {
                ui.label(egui::RichText::new("Disabled").weak());
            }
            ui.add_space(8.0);

            if !entry.description.is_empty() {
                let desc = entry.description.trim();
                skill_field_block(ui, "Description", desc, selected);
                ui.add_space(8.0);
            }

            let path_str = entry.path.display().to_string();
            skill_field_block(ui, "Path", &path_str, selected);
            ui.add_space(8.0);
        });
    });

    let click = ui.interact(
        frame_response.response.rect,
        frame_response.response.id,
        egui::Sense::click(),
    );
    if click.clicked() {
        if selected {
            app.selected_skill_name = None;
        } else {
            app.selected_skill_name = Some(entry.name.clone());
        }
    }
}
