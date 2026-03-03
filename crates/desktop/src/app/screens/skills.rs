use eframe::egui;

use crate::app::ChaiApp;

pub fn ui_skills_screen(app: &mut ChaiApp, ui: &mut egui::Ui) {
    const LINE_SPACING: f32 = 6.0;
    const SECTION_SPACING: f32 = 18.0;

    let (config, config_path) = lib::config::load_config(None)
        .unwrap_or((lib::config::Config::default(), std::path::PathBuf::new()));

    let skills_root = lib::config::resolve_skills_dir(&config, &config_path);
    let extra_dirs = config.skills.extra_dirs.clone();
    let enabled = config.skills.enabled.clone();

    let skills_result = lib::skills::load_skills(Some(skills_root.as_path()), &extra_dirs);
    let mut skills = match skills_result {
        Ok(list) => list,
        Err(e) => {
            let subtitle = format!("Values below are loaded from: {}", skills_root.display());
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
        let subtitle = format!("Values below are loaded from: {}", skills_root.display());
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

    let subtitle = format!("Values below are loaded from: {}", skills_root.display());

    crate::app::ui_screen(ui, "Skills", Some(&subtitle), |ui| {
            ui.style_mut().spacing.item_spacing.x = 24.0;

            ui.columns(2, |columns| {
                // Left column: skills list
                {
                    let ui_left = &mut columns[0];
                    egui::ScrollArea::vertical()
                        .id_source("skills_list_scroll")
                        .show(ui_left, |ui| {
                            // Enabled section
                            ui.label(egui::RichText::new("Enabled").strong());
                            ui.add_space(LINE_SPACING);
                            if enabled_skills.is_empty() {
                                ui.label("No skills enabled.");
                            } else {
                                for entry in enabled_skills.iter() {
                                    let title = entry.name.as_str();
                                    paint_one_skill(ui, app, entry, title, column_width_for_skills(ui));
                                    ui.add_space(SECTION_SPACING);
                                }
                            }
                            ui.add_space(SECTION_SPACING);

                            // Disabled section
                            ui.label(egui::RichText::new("Disabled").strong());
                            ui.add_space(LINE_SPACING);
                            if disabled_skills.is_empty() {
                                ui.label("No skills disabled.");
                            } else {
                                for entry in disabled_skills.iter() {
                                    let title = entry.name.as_str();
                                    paint_one_skill(ui, app, entry, title, column_width_for_skills(ui));
                                    ui.add_space(SECTION_SPACING);
                                }
                            }
                        });
                }

                // Right column: SKILL.md (top) + tools.json (bottom) for selected skill
                {
                    let ui_right = &mut columns[1];
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
                                ui.add_space(LINE_SPACING);

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

                        ui_right.add_space(SECTION_SPACING);

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
                                ui_right.add_space(LINE_SPACING);

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
                                ui_right.add_space(LINE_SPACING);
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

/// Returns (column_width, content_width) for the current left column.
/// Border aligns with headings; padding is inside the button on all sides.
fn column_width_for_skills(ui: &egui::Ui) -> (f32, f32) {
    let horizontal_padding = 8.0 * 2.0;
    let column_width = ui.available_width();
    let content_width = (column_width - horizontal_padding).max(0.0);
    (column_width, content_width)
}

fn paint_one_skill(
    ui: &mut egui::Ui,
    app: &mut ChaiApp,
    entry: &lib::skills::SkillEntry,
    title: &str,
    (column_width, content_width): (f32, f32),
) {
    // Padding between content and border on all sides (border still aligns with headings).
    let margin = egui::Margin {
        left: 8.0,
        right: 8.0,
        top: 8.0,
        bottom: 8.0,
    };

    let heading_font = egui::TextStyle::Button.resolve(ui.style());
    let body_font = egui::TextStyle::Body.resolve(ui.style());
    let path_text = format!("Path: {}", entry.path.display());

    let mut size_job = egui::text::LayoutJob::default();
    size_job.wrap.max_width = content_width;
    size_job.append(
        title,
        0.0,
        egui::text::TextFormat {
            font_id: heading_font.clone(),
            ..Default::default()
        },
    );
    size_job.append(
        "\n\n",
        0.0,
        egui::text::TextFormat {
            font_id: body_font.clone(),
            ..Default::default()
        },
    );
    if !entry.description.is_empty() {
        size_job.append(
            entry.description.trim(),
            0.0,
            egui::text::TextFormat {
                font_id: body_font.clone(),
                ..Default::default()
            },
        );
        size_job.append(
            "\n",
            0.0,
            egui::text::TextFormat {
                font_id: body_font.clone(),
                ..Default::default()
            },
        );
    }
    size_job.append(
        &path_text,
        0.0,
        egui::text::TextFormat {
            font_id: body_font.clone(),
            ..Default::default()
        },
    );

    let selected = app
        .selected_skill_name
        .as_deref()
        .map(|n| n == entry.name.as_str())
        .unwrap_or(false);

    let galley = ui.fonts(|f| f.layout_job(size_job));
    let desired_height = galley.size().y + margin.sum().y;
    let desired_size = egui::vec2(column_width, desired_height);

    let (outer_rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    // Inner rect: padding between content and border (inside the button).
    let content_rect = egui::Rect::from_min_max(
        outer_rect.min + egui::vec2(margin.left, margin.top),
        outer_rect.max - egui::vec2(margin.right, margin.bottom),
    );

    let visuals = ui.style().interact_selectable(&response, selected);
    let rounding = visuals.rounding;
    // Draw border on the full allocation so padding is inside the button.
    if selected {
        ui.painter().rect(outer_rect, rounding, visuals.bg_fill, visuals.bg_stroke);
    } else {
        let light_stroke = egui::Stroke {
            width: 1.0,
            color: ui.visuals().widgets.noninteractive.bg_stroke.color,
        };
        ui.painter().rect(outer_rect, rounding, egui::Color32::TRANSPARENT, light_stroke);
    }

    let heading_color = ui.visuals().strong_text_color();
    let body_color = if selected || response.hovered() {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };

    let mut paint_job = egui::text::LayoutJob::default();
    paint_job.wrap.max_width = content_width;
    paint_job.append(
        title,
        0.0,
        egui::text::TextFormat {
            font_id: heading_font,
            color: heading_color,
            ..Default::default()
        },
    );
    paint_job.append(
        "\n\n",
        0.0,
        egui::text::TextFormat {
            font_id: body_font.clone(),
            color: body_color,
            ..Default::default()
        },
    );
    if !entry.description.is_empty() {
        paint_job.append(
            entry.description.trim(),
            0.0,
            egui::text::TextFormat {
                font_id: body_font.clone(),
                color: body_color,
                ..Default::default()
            },
        );
        paint_job.append(
            "\n",
            0.0,
            egui::text::TextFormat {
                font_id: body_font.clone(),
                color: body_color,
                ..Default::default()
            },
        );
    }
    paint_job.append(
        &path_text,
        0.0,
        egui::text::TextFormat {
            font_id: body_font,
            color: body_color,
            ..Default::default()
        },
    );

    if ui.is_rect_visible(content_rect) {
        let paint_galley = ui.fonts(|f| f.layout_job(paint_job));
        ui.painter().galley(content_rect.left_top(), paint_galley);
    }

    if response.clicked() {
        app.selected_skill_name = Some(entry.name.clone());
    }
}

