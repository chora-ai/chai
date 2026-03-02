use eframe::egui;

use crate::app::ChaiApp;

pub fn ui_skills_screen(app: &mut ChaiApp, ui: &mut egui::Ui) {
    const LINE_SPACING: f32 = 6.0;
    const SECTION_SPACING: f32 = 18.0;
    /// Spacing between skill items (about half of SECTION_SPACING).
    const SKILL_ITEM_SPACING: f32 = 9.0;

    ui.add_space(24.0);
    ui.heading("Skills");
    ui.add_space(ChaiApp::SCREEN_TITLE_BOTTOM_SPACING);

    let (config, config_path) = lib::config::load_config(None)
        .unwrap_or((lib::config::Config::default(), std::path::PathBuf::new()));

    let skills_root = lib::config::resolve_skills_dir(&config, &config_path);
    let extra_dirs = config.skills.extra_dirs.clone();
    let enabled = config.skills.enabled.clone();

    ui.label(format!("Values below are loaded from: {}", skills_root.display()));
    ui.add_space(LINE_SPACING);
    if !extra_dirs.is_empty() {
        ui.label("Extra dirs:");
        ui.add_space(LINE_SPACING);
        for d in &extra_dirs {
            ui.label(format!("- {}", d.display()));
        }
        ui.add_space(LINE_SPACING);
    }
    ui.add_space(SECTION_SPACING);

    let skills_result = lib::skills::load_skills(Some(skills_root.as_path()), &extra_dirs);
    let mut skills = match skills_result {
        Ok(list) => list,
        Err(e) => {
            ui.colored_label(
                egui::Color32::RED,
                format!("failed to load skills: {}", e),
            );
            ui.add_space(ChaiApp::SCREEN_FOOTER_SPACING);
            return;
        }
    };

    if skills.is_empty() {
        ui.label("No skills found in the configured directories.");
        ui.add_space(ChaiApp::SCREEN_FOOTER_SPACING);
        return;
    }

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    let enabled_set: std::collections::HashSet<String> =
        enabled.into_iter().map(|s| s.trim().to_string()).collect();
    let enabled_skills: Vec<_> = skills.iter().filter(|e| enabled_set.contains(e.name.as_str())).collect();
    let disabled_skills: Vec<_> = skills.iter().filter(|e| !enabled_set.contains(e.name.as_str())).collect();

    // Two-column layout: left = skills list (Enabled / Disabled), right = tools.json for selected skill
    let available = ui.available_height();
    let content_height = (available - ChaiApp::SCREEN_FOOTER_SPACING).max(0.0);
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), content_height),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            // Ensure the gutter between the two columns matches the horizontal padding
            // used elsewhere in the app (24.0 on each side).
            let old_spacing = ui.style().spacing.item_spacing.x;
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
                                    ui.add_space(SKILL_ITEM_SPACING);
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
                                    ui.add_space(SKILL_ITEM_SPACING);
                                }
                            }
                        });
                }

                // Right column: tools.json for selected skill
                {
                    let ui_right = &mut columns[1];
                    ui_right.label(egui::RichText::new("Tools").strong());
                    ui_right.add_space(LINE_SPACING);

                    let selected = app
                        .selected_skill_name
                        .as_ref()
                        .and_then(|name| skills.iter().find(|e| &e.name == name));

                    if let Some(entry) = selected {
                        let tools_path = entry.path.join("tools.json");
                        match std::fs::read_to_string(&tools_path) {
                            Ok(contents) => {
                                // Pretty-print tools.json with two-space indentation when possible.
                                let pretty = serde_json::from_str::<serde_json::Value>(&contents)
                                    .ok()
                                    .and_then(|value| serde_json::to_string_pretty(&value).ok())
                                    .unwrap_or(contents);

                                egui::ScrollArea::vertical()
                                    .id_source("skills_tools_scroll")
                                    .max_height(ui_right.available_height())
                                    .show(ui_right, |ui| {
                                        let mut buf = pretty.clone();
                                        egui::TextEdit::multiline(&mut buf)
                                            .code_editor()
                                            .desired_width(ui.available_width())
                                            .interactive(false)
                                            .show(ui);
                                    });
                            }
                            Err(_) => {
                                ui_right.label("No tools.json found for this skill.");
                            }
                        }
                    } else {
                        ui_right.label("Select a skill to view its tools.json.");
                    }
                }
            });

            ui.style_mut().spacing.item_spacing.x = old_spacing;
        },
    );

    ui.add_space(ChaiApp::SCREEN_FOOTER_SPACING);
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

