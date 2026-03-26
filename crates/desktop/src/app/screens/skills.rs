use eframe::egui;

use crate::app::ui::{dashboard, spacing};
use crate::app::ChaiApp;

pub fn ui_skills_screen(app: &mut ChaiApp, ui: &mut egui::Ui) {

    let (config, config_path) = lib::config::load_config(None)
        .unwrap_or((lib::config::Config::default(), std::path::PathBuf::new()));

    let skills_root = lib::config::resolve_skills_dir(&config, &config_path);
    let extra_dirs = config.skills.extra_dirs.clone();
    let enabled = config.skills.enabled.clone();

    let skills_result = lib::skills::load_skills(Some(skills_root.as_path()), &extra_dirs);
    let mut skills = match skills_result {
        Ok(list) => list,
        Err(e) => {
            let subtitle = format!("Values below are loaded from {}", skills_root.display());
            crate::app::ui_screen(ui, "Skills", Some(&subtitle), |ui| {
                ui.colored_label(
                    egui::Color32::RED,
                    format!("failed to load skills: {}", e),
                );
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
    let enabled_skills: Vec<_> =
        skills.iter().filter(|e| enabled_set.contains(e.name.as_str())).collect();
    let disabled_skills: Vec<_> =
        skills.iter().filter(|e| !enabled_set.contains(e.name.as_str())).collect();

    let subtitle = format!("Values below are loaded from {}", skills_root.display());

    crate::app::ui_screen(ui, "Skills", Some(&subtitle), |ui| {
            dashboard::dashboard_two_columns(ui, |ui_left, ui_right| {
                // Left column: skills list
                {
                    egui::ScrollArea::vertical()
                        .id_source("skills_list_scroll")
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

/// [`dashboard::kv`] layout with white key and value (selected skill card on selection fill).
fn skill_kv_row_selected(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.horizontal_top(|ui| {
        let gap = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = spacing::KV_KEY_VALUE_GAP;
        ui.allocate_ui_with_layout(
            egui::vec2(spacing::KV_LABEL_COLUMN_WIDTH, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.label(
                    egui::RichText::new(format!("{}:", key)).color(egui::Color32::WHITE),
                );
            },
        );
        ui.add(egui::Label::new(
            egui::RichText::new(value).color(egui::Color32::WHITE),
        ));
        ui.spacing_mut().item_spacing.x = gap;
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
                egui::RichText::new(title).strong().color(egui::Color32::WHITE)
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
            let (sep_rect, _) = ui.allocate_exact_size(egui::vec2(sep_w, 1.0), egui::Sense::hover());
            ui.painter()
                .hline(sep_rect.x_range(), sep_rect.center().y, sep_stroke);
            ui.add_space(spacing::GROUP_AFTER_SEPARATOR);

            if !entry.description.is_empty() {
                let desc = entry.description.trim();
                if selected {
                    skill_kv_row_selected(ui, "Description", desc);
                } else {
                    dashboard::kv(ui, "Description", desc);
                }
            }

            let path_str = entry.path.display().to_string();
            if selected {
                skill_kv_row_selected(ui, "Path", &path_str);
            } else {
                dashboard::kv(ui, "Path", &path_str);
            }
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

