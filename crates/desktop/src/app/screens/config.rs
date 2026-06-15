use eframe::egui;

use crate::app::ui::{dashboard, readonly_code, spacing, view_toggle};
use crate::app::{ChaiApp, ConfigViewMode};

pub fn ui_config_screen(app: &mut ChaiApp, ui: &mut egui::Ui) {
    app.invalidate_enabled_providers_cache();
    let Ok((config, paths)) = lib::config::load_config(app.effective_profile_override()) else {
        crate::app::ui_screen(ui, "Config", None, |ui| {
            ui.label(egui::RichText::new("could not load profile (run `chai init`)").weak());
        });
        return;
    };
    let config_path = paths.config_path.clone();
    if app.default_model.is_none() {
        let (_, model) = lib::config::resolve_effective_provider_and_model(&config.providers, &config.agents);
        app.default_model = Some(model);
    }

    let subtitle = if !config_path.as_os_str().is_empty() {
        Some(format!(
            "Values loaded from {}",
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

                config_summary_dashboard(ui, &config, paths.profile_dir.as_path());
            });
    });
}

fn config_summary_dashboard(
    ui: &mut egui::Ui,
    config: &lib::config::Config,
    profile_dir: &std::path::Path,
) {
    dashboard::dashboard_two_columns(ui, |left, right| {
        left.vertical(|ui| {
            config_summary_left_column(ui, config);
        });
        right.vertical(|ui| {
            config_summary_right_column(ui, config, profile_dir);
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
                    if t.trim().is_empty() {
                        "(empty)"
                    } else {
                        "set"
                    },
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
        if config.providers.entries.is_empty() {
            ui.label(egui::RichText::new("Not configured.").weak());
            return;
        }

        for def in &config.providers.entries {
            let resolved_base = lib::config::resolve_provider_base_url(&config.providers, &def.id);
            let resolved_key = lib::config::resolve_provider_api_key(&config.providers, &def.id);

            ui.add_space(spacing::SUBSECTION_HEADING_GAP);
            // Show provider id with endpoint type label.
            let endpoint_label = match def.endpoint_type {
                lib::config::EndpointType::Ollama => "Ollama",
                lib::config::EndpointType::OpenaiCompat => "OpenAI-Compatible",
            };
            ui.label(egui::RichText::new(format!("{} ({})", def.id, endpoint_label)).strong());
            ui.add_space(spacing::SUBSECTION_HEADING_GAP);
            dashboard::kv(ui, "endpoint type", def.endpoint_type.as_str());
            if let Some(ref url) = resolved_base {
                dashboard::kv(ui, "base URL", url.as_str());
            }
            let key_set = resolved_key
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            dashboard::kv(ui, "API key", if key_set { "set" } else { "(not set)" });
            if let Some(ref default_model) = def.default_model {
                if !default_model.trim().is_empty() {
                    dashboard::kv(ui, "default model", default_model.trim());
                }
            }
            // Model discovery.
            let discovery_label = match def.model_discovery {
                lib::config::ModelDiscovery::Default => "default",
                lib::config::ModelDiscovery::Lmstudio => "lmstudio",
                lib::config::ModelDiscovery::Static => "static",
            };
            dashboard::kv(ui, "model discovery", discovery_label);
            if !def.static_models.is_empty() {
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                ui.label(egui::RichText::new("Static models"));
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                ui.horizontal_wrapped(|ui| {
                    for m in &def.static_models {
                        ui.add(egui::Label::new(egui::RichText::new(m)));
                    }
                });
                ui.add_space(spacing::TABLE_BLOCK_AFTER);
            }
            // Auto-load.
            let autoload_label = match def.auto_load {
                lib::config::AutoLoad::None => "off",
                lib::config::AutoLoad::Lmstudio => "lmstudio",
            };
            dashboard::kv(ui, "auto load", autoload_label);
        }
    });
}

fn enabled_providers_display(opt: &Option<Vec<String>>) -> String {
    let v: Vec<String> = opt
        .as_ref()
        .map(|list| {
            list.iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    if v.is_empty() {
        "(none)".to_string()
    } else {
        v.join(", ")
    }
}

fn config_summary_right_column(
    ui: &mut egui::Ui,
    config: &lib::config::Config,
    profile_dir: &std::path::Path,
) {
    let (default_provider, default_model) =
        lib::config::resolve_effective_provider_and_model(&config.providers, &config.agents);

    let orch_id = config
        .agents
        .orchestrator_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("orchestrator");

    dashboard::section_group(ui, "Agents", |ui| {
        ui.label(egui::RichText::new(format!("Orchestrator: {}", orch_id)).strong());
        ui.add_space(spacing::SUBSECTION_HEADING_GAP);
        dashboard::kv(ui, "Default provider", default_provider.as_str());
        dashboard::kv(ui, "Default model", default_model.as_str());
        let orch_ep = enabled_providers_display(&config.agents.enabled_providers);
        dashboard::kv(ui, "enabledProviders", orch_ep.as_str());
        let orch_dir = lib::config::orchestrator_context_dir(config, profile_dir);
        dashboard::kv(ui, "Context directory", &orch_dir.display().to_string());
        let orch_skills = lib::config::orchestrator_skills_enabled_list(&config.agents);
        let orch_skills_csv = orch_skills.join(", ");
        dashboard::kv(
            ui,
            "skillsEnabled",
            if orch_skills.is_empty() {
                "(none)"
            } else {
                orch_skills_csv.as_str()
            },
        );
        let orch_mode = lib::config::orchestrator_context_mode(&config.agents);
        dashboard::kv(
            ui,
            "contextMode",
            match orch_mode {
                lib::config::SkillContextMode::Full => "full",
                lib::config::SkillContextMode::ReadOnDemand => "readOnDemand",
            },
        );

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
                ui.label(egui::RichText::new(
                    "Max delegations per provider (session)",
                ));
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

        if let Some(ref workers) = config.agents.workers {
            for w in workers {
                let wid = w.id.trim();
                if wid.is_empty() {
                    continue;
                }
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                ui.label(egui::RichText::new(format!("Worker: {}", wid)).strong());
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                let (wp, wm) = lib::orchestration::effective_worker_defaults(&config.providers, &config.agents, w);
                dashboard::kv(ui, "Default provider", wp.as_str());
                dashboard::kv(ui, "Default model", wm.as_str());
                let w_dir = lib::config::worker_context_dir(w, profile_dir);
                dashboard::kv(
                    ui,
                    "Context directory",
                    &w_dir
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "(unknown)".to_string()),
                );
                let w_skills = lib::config::worker_skills_enabled_list(w);
                let w_skills_csv = w_skills.join(", ");
                dashboard::kv(
                    ui,
                    "skillsEnabled",
                    if w_skills.is_empty() {
                        "(none)"
                    } else {
                        w_skills_csv.as_str()
                    },
                );
                let w_mode = lib::config::worker_context_mode(w);
                dashboard::kv(
                    ui,
                    "contextMode",
                    match w_mode {
                        lib::config::SkillContextMode::Full => "full",
                        lib::config::SkillContextMode::ReadOnDemand => "readOnDemand",
                    },
                );
            }
        }
    });
}
