use eframe::egui;

use crate::app::ui::{dashboard, spacing};
use crate::app::ChaiApp;

pub fn ui_context_screen(app: &mut ChaiApp, ui: &mut egui::Ui, running: bool) {
    crate::app::ui_screen(
        ui,
        "Context",
        Some(if running {
            "Values below are loaded from the gateway status."
        } else {
            "Start the gateway to load the system context."
        }),
        |ui| {
            if !running {
                return;
            }

            let total_height = ui.available_height();
            // Prefer live status `contextMode`; fall back to config when status not yet received.
            let is_read_on_demand =
                if let Some(mode) = app
                    .gateway_status
                    .as_ref()
                    .and_then(|s| s.context_mode.as_deref())
                {
                    mode == "readOnDemand"
                } else {
                    let (config, _) = lib::config::load_config(None).unwrap_or((
                        lib::config::Config::default(),
                        std::path::PathBuf::new(),
                    ));
                    matches!(
                        config.skills.context_mode,
                        lib::config::SkillContextMode::ReadOnDemand
                    )
                };

            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), total_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let context_text = app
                        .gateway_status
                        .as_ref()
                        .and_then(|s| s.system_context.as_deref())
                        .filter(|s| !s.trim().is_empty());
                    // Do not treat background status refetch as loading — keep showing cached context.
                    let loading = app.gateway_status.is_none();

                    if is_read_on_demand {
                        // Two columns: system context + per-skill bodies (merged tools live on **Tools**).
                        dashboard::dashboard_two_columns(ui, |ui_left, ui_right| {
                            if let Some(text) = context_text {
                                ui_left.label(egui::RichText::new("System Context").strong());
                                ui_left.add_space(spacing::LINE);
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
                            } else if loading {
                                ui_left.label("Loading from gateway status...");
                            } else {
                                ui_left.label("No context loaded.");
                            }

                            let loading_skills = app.gateway_status.is_none();

                            if !loading_skills {
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
                                }
                            }
                        });
                    } else {
                        // Full mode: single column — system context only (tools on **Tools** screen).
                        ui.label(egui::RichText::new("System Context").strong());
                        ui.add_space(spacing::LINE);

                        if let Some(text) = context_text {
                            let mut buf = text.to_string();
                            let scroll_height = ui.available_height();
                            egui::ScrollArea::vertical()
                                .id_source("context_system_scroll_full")
                                .max_height(scroll_height)
                                .show(ui, |ui| {
                                    egui::TextEdit::multiline(&mut buf)
                                        .code_editor()
                                        .desired_width(ui.available_width())
                                        .interactive(false)
                                        .show(ui);
                                });
                        } else if loading {
                            ui.label("(loading context)");
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
