use eframe::egui;

use crate::app::ChaiApp;

pub fn ui_context_screen(app: &mut ChaiApp, ui: &mut egui::Ui, running: bool) {
    const INFO_LINE_SPACING: f32 = 6.0;
    const INFO_SUBSECTION_SPACING: f32 = 18.0;

    ui.add_space(24.0);
    ui.heading("Context");
    ui.add_space(ChaiApp::SCREEN_TITLE_BOTTOM_SPACING);
    ui.label("Values below are loaded from the running gateway (system context).");
    ui.add_space(INFO_LINE_SPACING);
    ui.add_space(INFO_SUBSECTION_SPACING);

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
                // Left column: system context
                {
                    let ui_left = &mut columns[0];
                    ui_left.label(egui::RichText::new("System Context").strong());
                    // Base spacing under the subheader, even when there is no content.
                    ui_left.add_space(INFO_LINE_SPACING);

                    let context_text = app
                        .gateway_status
                        .as_ref()
                        .and_then(|s| s.system_context.as_deref())
                        .filter(|s| !s.trim().is_empty());
                    let loading = !running
                        || app.gateway_status.is_none()
                        || app.status_receiver.is_some();

                    if let Some(text) = context_text {
                        let mut buf = text.to_string();
                        let scroll_height = ui_left.available_height();
                        egui::ScrollArea::vertical()
                            .id_source("context_system_scroll")
                            .max_height(scroll_height)
                            .show(ui_left, |ui| {
                                egui::TextEdit::multiline(&mut buf)
                                    .code_editor()
                                    .desired_width(ui.available_width())
                                    .interactive(false)
                                    .show(ui);
                            });
                    } else if !running {
                        ui_left.label("(start gateway to see context)");
                    } else if loading {
                        ui_left.label("(loading context)");
                    } else {
                        ui_left.label("No context loaded.");
                    }
                }

                // Right column: skills context (only when read-on-demand)
                {
                    let ui_right = &mut columns[1];
                    // Only treat as "loading" when we don't have status yet. Skills content is
                    // loaded from disk, not from the status response, so we keep showing it
                    // during periodic status refetches (status_receiver.is_some()) to avoid
                    // flashing "(loading skills)".
                    let loading_skills = !running || app.gateway_status.is_none();
                    let context_mode = app
                        .gateway_status
                        .as_ref()
                        .and_then(|s| s.context_mode.as_deref());

                    if context_mode == Some("readOnDemand") {
                        if loading_skills {
                            ui_right.label("(loading skills)");
                        } else {
                            // Load config and skills so we can show per-skill context bodies,
                            // matching what read_skill(skill_name) returns.
                            let (config, config_path) =
                                lib::config::load_config(None).unwrap_or((
                                    lib::config::Config::default(),
                                    std::path::PathBuf::new(),
                                ));

                            if !config.skills.enabled.is_empty() {
                                let skills_root =
                                    lib::config::resolve_skills_dir(&config, &config_path);
                                match lib::skills::load_skills(
                                    Some(skills_root.as_path()),
                                    &config.skills.extra_dirs,
                                ) {
                                    Ok(mut entries) => {
                                        // Keep only enabled skills, as the gateway does for agent context.
                                        entries.retain(|e| {
                                            config.skills.enabled.iter().any(|n| n == &e.name)
                                        });
                                        if entries.is_empty() {
                                            ui_right.label("No enabled skills were loaded.");
                                        } else {
                                            entries.sort_by(|a, b| a.name.cmp(&b.name));
                                            egui::ScrollArea::vertical()
                                                .id_source("context_skills_scroll")
                                                .max_height(ui_right.available_height())
                                                .show(ui_right, |ui| {
                                                    for entry in &entries {
                                                        let body =
                                                            strip_skill_frontmatter(&entry.content);
                                                        let has_body = !body.trim().is_empty();

                                                        ui.label(
                                                            egui::RichText::new(&entry.name)
                                                                .strong(),
                                                        );
                                                        // Base spacing below subheader, always.
                                                        ui.add_space(INFO_LINE_SPACING);

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

                                                        // Spacing before the next skill (or end).
                                                        ui.add_space(INFO_SUBSECTION_SPACING);
                                                    }
                                                });
                                        }
                                    }
                                    Err(e) => {
                                        ui_right.colored_label(
                                            egui::Color32::RED,
                                            format!("failed to load skills: {}", e),
                                        );
                                        ui_right.add_space(INFO_LINE_SPACING);
                                    }
                                }
                            }
                        }
                    }
                }
            });

            ui.style_mut().spacing.item_spacing.x = old_spacing;
        },
    );
    ui.add_space(ChaiApp::SCREEN_FOOTER_SPACING);
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
        // Use empty slice when i+4 is out of bounds (e.g. multibyte chars before "\n---")
        // so we don't return the closing delimiter as body.
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


