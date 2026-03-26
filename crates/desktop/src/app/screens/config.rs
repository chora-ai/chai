use eframe::egui;

use crate::app::ui::{dashboard, readonly_code, spacing, view_toggle};
use crate::app::{ChaiApp, ConfigViewMode};

pub fn ui_config_screen(app: &mut ChaiApp, ui: &mut egui::Ui) {
    app.invalidate_enabled_providers_cache();
    let (config, config_path) = lib::config::load_config(None)
        .unwrap_or((lib::config::Config::default(), std::path::PathBuf::new()));
    if app.default_model.is_none() {
        let (_, model) = lib::config::resolve_effective_provider_and_model(&config.agents);
        app.default_model = Some(model);
    }

    let subtitle = if !config_path.as_os_str().is_empty() {
        Some(format!(
            "Values below are loaded from {}",
            config_path.display()
        ))
    } else {
        None
    };

    crate::app::ui_screen(ui, "Config", subtitle.as_deref(), |ui| {
        view_toggle::config_view_radios(ui, &mut app.config_view_mode);
        ui.add_space(spacing::SUBSECTION);

        egui::ScrollArea::vertical()
            .max_height(ui.available_height().max(80.0))
            .show(ui, |ui| {
                if app.config_view_mode == ConfigViewMode::RawJson {
                    if config_path.as_os_str().is_empty() {
                        ui.label(egui::RichText::new("(no config path resolved)").weak());
                        return;
                    }
                    if !config_path.exists() {
                        app.config_raw_display_buffer.clear();
                        ui.label(
                            egui::RichText::new(format!(
                                "No file at {} — the Dashboard view shows defaults until you create it.",
                                config_path.display()
                            ))
                            .weak(),
                        );
                        return;
                    }
                    match std::fs::read_to_string(&config_path) {
                        Ok(s) => {
                            if app.config_raw_display_buffer.as_str() != s.as_str() {
                                app.config_raw_display_buffer = s;
                            }
                            // `ui` is already inside an outer `ScrollArea` in this screen, so use the
                            // non-scroll variant to avoid nested scrollbars.
                            readonly_code::read_only_code_block(
                                ui,
                                "config_raw_textedit",
                                &mut app.config_raw_display_buffer,
                                28,
                            );
                        }
                        Err(e) => {
                            ui.colored_label(
                                egui::Color32::RED,
                                format!("failed to read {}: {}", config_path.display(), e),
                            );
                        }
                    }
                    return;
                }

                config_summary_dashboard(ui, &config);
            });
    });
}

fn config_summary_dashboard(ui: &mut egui::Ui, config: &lib::config::Config) {
    dashboard::dashboard_two_columns(ui, |left, right| {
        left.vertical(|ui| {
            config_summary_left_column(ui, config);
        });
        right.vertical(|ui| {
            config_summary_right_column(ui, config);
        });
    });
}

fn config_summary_left_column(ui: &mut egui::Ui, config: &lib::config::Config) {
    dashboard::section_group(ui, "Gateway", |ui| {
        let auth_mode = match config.gateway.auth.mode {
            lib::config::GatewayAuthMode::None => "none",
            lib::config::GatewayAuthMode::Token => "token",
        };
        dashboard::kv(ui, "Bind", config.gateway.bind.trim());
        dashboard::kv(ui, "Port", &config.gateway.port.to_string());
        dashboard::kv(ui, "Auth", auth_mode);
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);

    dashboard::section_group(ui, "Channels", |ui| {
        let telegram_configured = config.channels.telegram.bot_token.is_some()
            || config.channels.telegram.webhook_url.is_some();
        if telegram_configured {
            if let Some(ref t) = config.channels.telegram.bot_token {
                dashboard::kv(
                    ui,
                    "Telegram bot token",
                    if t.trim().is_empty() { "(empty)" } else { "set" },
                );
            }
            if let Some(ref w) = config.channels.telegram.webhook_url {
                dashboard::kv(ui, "Telegram webhook", w.as_str());
            }
        } else {
            ui.label(egui::RichText::new("Telegram: not configured.").weak());
            ui.add_space(spacing::LINE);
        }

        let matrix_configured = config
            .channels
            .matrix
            .homeserver
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
            && (config
                .channels
                .matrix
                .access_token
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
                || (config
                    .channels
                    .matrix
                    .user
                    .as_ref()
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false)
                    && config
                        .channels
                        .matrix
                        .password
                        .as_ref()
                        .map(|s| !s.trim().is_empty())
                        .unwrap_or(false)));
        if matrix_configured {
            if let Some(ref h) = config.channels.matrix.homeserver {
                dashboard::kv(ui, "Matrix homeserver", h.as_str());
            }
            let mode = if config
                .channels
                .matrix
                .access_token
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
            {
                "access token set"
            } else {
                "password login"
            };
            dashboard::kv(ui, "Matrix", mode);
        } else {
            ui.label(egui::RichText::new("Matrix: not configured.").weak());
            ui.add_space(spacing::LINE);
        }

        let signal_configured = config
            .channels
            .signal
            .http_base
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if signal_configured {
            if let Some(ref h) = config.channels.signal.http_base {
                dashboard::kv(ui, "Signal (signal-cli HTTP)", h.as_str());
            }
            if let Some(ref a) = config.channels.signal.account {
                dashboard::kv(ui, "Signal account", a.as_str());
            }
        } else {
            ui.label(egui::RichText::new("Signal: not configured.").weak());
        }
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);

    dashboard::section_group(ui, "Providers", |ui| {
        let Some(ref b) = config.providers else {
            ui.label(egui::RichText::new("Not configured.").weak());
            return;
        };

        let mut any = false;

        if let Some(ref o) = b.ollama {
            if let Some(url) = o.base_url.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                dashboard::kv(ui, "Ollama base URL", url);
                any = true;
            }
        }
        if let Some(ref l) = b.lms {
            if let Some(url) = l.base_url.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                dashboard::kv(ui, "LM Studio base URL", url);
                any = true;
            }
        }
        if let Some(ref n) = b.nim {
            let key_set = n
                .api_key
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            dashboard::kv(
                ui,
                "NIM API key",
                if key_set { "set" } else { "(not set)" },
            );
            any = true;
            if let Some(ref extra) = n.extra_models {
                if !extra.is_empty() {
                    ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                    ui.label(egui::RichText::new("NIM extra models"));
                    ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                    ui.horizontal_wrapped(|ui| {
                        for m in extra {
                            ui.add(
                                egui::Label::new(egui::RichText::new(m)),
                            );
                        }
                    });
                    ui.add_space(spacing::TABLE_BLOCK_AFTER);
                }
            }
        }
        if let Some(ref v) = b.vllm {
            if let Some(url) = v.base_url.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                dashboard::kv(ui, "vLLM base URL", url);
            }
            let key_set = v
                .api_key
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            dashboard::kv(
                ui,
                "vLLM API key",
                if key_set { "set" } else { "(not set)" },
            );
            any = true;
        }
        if let Some(ref o) = b.openai {
            if let Some(url) = o.base_url.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                dashboard::kv(ui, "OpenAI base URL", url);
            }
            let key_set = o
                .api_key
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            dashboard::kv(
                ui,
                "OpenAI API key",
                if key_set { "set" } else { "(not set)" },
            );
            any = true;
        }
        if let Some(ref h) = b.hf {
            if let Some(url) = h.base_url.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                dashboard::kv(ui, "Hugging Face base URL", url);
            }
            let key_set = h
                .api_key
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            dashboard::kv(
                ui,
                "Hugging Face API key",
                if key_set { "set" } else { "(not set)" },
            );
            any = true;
        }

        if !any {
            ui.label(egui::RichText::new("Not configured.").weak());
        }
    });
}

fn config_summary_right_column(ui: &mut egui::Ui, config: &lib::config::Config) {
    let (default_provider, default_model) =
        lib::config::resolve_effective_provider_and_model(&config.agents);

    dashboard::section_group(ui, "Agents", |ui| {
        dashboard::kv(ui, "Default provider", default_provider.as_str());
        dashboard::kv(ui, "Default model", default_model.as_str());
        if let Some(ref oid) = config.agents.orchestrator_id {
            dashboard::kv(ui, "Orchestrator id", oid.as_str());
        }

        let mut enabled: Vec<String> = config
            .agents
            .enabled_providers
            .as_ref()
            .map(|v| {
                v.iter()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        if enabled.is_empty() {
            enabled.push(default_provider.to_string());
        }
        let enabled_csv = enabled.join(", ");
        dashboard::kv(ui, "Enabled providers", &enabled_csv);

        match lib::config::resolve_workspace_dir(config) {
            Some(ref path) => {
                dashboard::kv(ui, "Workspace (effective)", &path.display().to_string());
                if config.agents.workspace.is_none() {
                    ui.label(
                        egui::RichText::new("(config default — agents.workspace not set)").weak(),
                    );
                    ui.add_space(spacing::LINE);
                }
            }
            None => {
                dashboard::kv(ui, "Workspace (effective)", "(not resolved)");
            }
        }

        if let Some(ref workers) = config.agents.workers {
            if !workers.is_empty() {
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                ui.label(egui::RichText::new("Workers").strong());
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                egui::Grid::new("config_workers_grid")
                    .num_columns(3)
                    .striped(true)
                    .spacing([spacing::GRID_CELL_SPACING, spacing::GRID_CELL_SPACING])
                    .show(ui, |ui| {
                        dashboard::grid_cell(ui, |ui| {
                            ui.label(dashboard::grid_header_rich(ui, "Worker"));
                        });
                        dashboard::grid_cell(ui, |ui| {
                            ui.label(dashboard::grid_header_rich(ui, "Default provider"));
                        });
                        dashboard::grid_cell(ui, |ui| {
                            ui.label(dashboard::grid_header_rich(ui, "Default model"));
                        });
                        ui.end_row();
                        for w in workers {
                            let (wp, wm) =
                                lib::orchestration::effective_worker_defaults(&config.agents, w);
                            dashboard::grid_cell(ui, |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(w.id.trim())));
                            });
                            dashboard::grid_cell(ui, |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(wp)));
                            });
                            dashboard::grid_cell(ui, |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(wm)));
                            });
                            ui.end_row();
                        }
                    });
                ui.add_space(spacing::TABLE_BLOCK_AFTER);
            }
        }

        let mut any_limit = false;
        if let Some(n) = config.agents.max_session_messages {
            dashboard::kv(ui, "Max session messages", &n.to_string());
            any_limit = true;
        }
        if let Some(n) = config.agents.max_delegations_per_turn {
            dashboard::kv(ui, "Max delegations per turn", &n.to_string());
            any_limit = true;
        }
        if let Some(n) = config.agents.max_delegations_per_session {
            dashboard::kv(ui, "Max delegations per session", &n.to_string());
            any_limit = true;
        }
        if let Some(ref m) = config.agents.max_delegations_per_provider {
            if !m.is_empty() {
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                ui.label(egui::RichText::new("Max delegations per provider (session)"));
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                for (k, v) in m {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(k.as_str()));
                        ui.label(egui::RichText::new("→").weak());
                        ui.label(egui::RichText::new(v.to_string()));
                    });
                }
                any_limit = true;
            }
        }
        if any_limit {
            ui.add_space(spacing::LINE);
        }

        if let Some(ref v) = config.agents.delegate_blocked_providers {
            if !v.is_empty() {
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                ui.label(egui::RichText::new("Delegation blocked providers"));
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                ui.horizontal_wrapped(|ui| {
                    for p in v {
                        ui.add(
                            egui::Label::new(egui::RichText::new(p)),
                        );
                    }
                });
                ui.add_space(spacing::TABLE_BLOCK_AFTER);
            }
        }

        if let Some(ref routes) = config.agents.delegation_instruction_routes {
            if !routes.is_empty() {
                ui.add_space(spacing::COLLAPSING_HEADER_BEFORE);
                egui::CollapsingHeader::new(format!(
                    "Delegation instruction routes ({} routes)",
                    routes.len()
                ))
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(spacing::COLLAPSING_BODY_INSET);
                    egui::Grid::new("config_delegation_routes_grid")
                        .num_columns(4)
                        .striped(true)
                        .spacing([spacing::GRID_CELL_SPACING, spacing::GRID_CELL_SPACING])
                        .show(ui, |ui| {
                            dashboard::grid_cell(ui, |ui| {
                                ui.label(dashboard::grid_header_rich(ui, "Prefix"));
                            });
                            dashboard::grid_cell(ui, |ui| {
                                ui.label(dashboard::grid_header_rich(ui, "Worker"));
                            });
                            dashboard::grid_cell(ui, |ui| {
                                ui.label(dashboard::grid_header_rich(ui, "Provider"));
                            });
                            dashboard::grid_cell(ui, |ui| {
                                ui.label(dashboard::grid_header_rich(ui, "Model"));
                            });
                            ui.end_row();
                            for r in routes {
                                dashboard::grid_cell(ui, |ui| {
                                    ui.add(egui::Label::new(
                                        egui::RichText::new(r.instruction_prefix.trim()),
                                    ));
                                });

                                let worker_id = r
                                    .worker_id
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or("—");
                                dashboard::grid_cell(ui, |ui| {
                                    ui.add(egui::Label::new(egui::RichText::new(worker_id)));
                                });

                                let provider = r
                                    .provider
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or("—");
                                dashboard::grid_cell(ui, |ui| {
                                    ui.add(egui::Label::new(egui::RichText::new(provider)));
                                });

                                let model = r
                                    .model
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or("—");
                                dashboard::grid_cell(ui, |ui| {
                                    ui.add(egui::Label::new(egui::RichText::new(model)));
                                });
                                ui.end_row();
                            }
                        });
                    ui.add_space(spacing::COLLAPSING_BODY_INSET);
                });
                ui.add_space(spacing::TABLE_BLOCK_AFTER);
            }
        }

        if let Some(ref allowed) = config.agents.delegate_allowed_models {
            if !allowed.is_empty() {
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                ui.label(
                    egui::RichText::new("Delegate allowed models (orchestrator)").strong(),
                );
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                egui::Grid::new("config_allowed_models_grid")
                    .num_columns(2)
                    .striped(true)
                    .spacing([spacing::GRID_CELL_SPACING, spacing::GRID_CELL_SPACING])
                    .show(ui, |ui| {
                        dashboard::grid_cell(ui, |ui| {
                            ui.label(dashboard::grid_header_rich(ui, "Provider"));
                        });
                        dashboard::grid_cell(ui, |ui| {
                            ui.label(dashboard::grid_header_rich(ui, "Model"));
                        });
                        ui.end_row();
                        for e in allowed {
                            dashboard::grid_cell(ui, |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(&e.provider)));
                            });
                            dashboard::grid_cell(ui, |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(&e.model)));
                            });
                            ui.end_row();
                        }
                    });
                ui.add_space(spacing::TABLE_BLOCK_AFTER);
            }
        }
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);

    dashboard::section_group(ui, "Skills", |ui| {
        let skills_configured = config.skills.directory.is_some()
            || !config.skills.extra_dirs.is_empty()
            || !config.skills.enabled.is_empty()
            || config.skills.context_mode != lib::config::SkillContextMode::Full;
        if skills_configured {
            if let Some(ref d) = config.skills.directory {
                dashboard::kv(ui, "Directory", &d.display().to_string());
            }
            if !config.skills.extra_dirs.is_empty() {
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                ui.label(egui::RichText::new("Extra dirs"));
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                ui.horizontal_wrapped(|ui| {
                    for p in &config.skills.extra_dirs {
                        ui.add(egui::Label::new(egui::RichText::new(
                            p.display().to_string(),
                        )));
                    }
                });
                ui.add_space(spacing::TABLE_BLOCK_AFTER);
            }
            if !config.skills.enabled.is_empty() {
                let enabled_csv = config.skills.enabled.join(", ");
                dashboard::kv(ui, "Enabled", &enabled_csv);
            }
            let context_mode_str = match config.skills.context_mode {
                lib::config::SkillContextMode::Full => "full",
                lib::config::SkillContextMode::ReadOnDemand => "readOnDemand",
            };
            dashboard::kv(ui, "Context mode", context_mode_str);
        } else {
            ui.label(egui::RichText::new("Not configured.").weak());
        }
    });
}
