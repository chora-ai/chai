use eframe::egui;

use crate::app::ui::{dashboard, readonly_code, spacing, view_toggle};
use crate::app::{ChaiApp, ConfigViewMode};

pub fn ui_config_screen(app: &mut ChaiApp, ui: &mut egui::Ui) {
    app.invalidate_enabled_providers_cache();
    let (config, paths) = match app.load_config_cached() {
        Ok(cp) => (cp.0.clone(), cp.1.clone()),
        Err(e) => {
            crate::app::ui_screen(ui, "Config", Some("Fix the error below to load config."), |ui| {
                ui.colored_label(egui::Color32::RED, format!("failed to load config: {}", e));
            });
            return;
        }
    };
    let config_path = paths.config_path.clone();
    if app.default_model().is_none() {
        let (_, model) = lib::config::resolve_effective_provider_and_model(&config.providers, &config.agents);
        *app.default_model_mut() = Some(model);
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
                                "No file at {} — the Dashboard view shows defaults.",
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

fn config_summary_dashboard(
    ui: &mut egui::Ui,
    config: &lib::config::Config,
) {
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
        ui.add_space(spacing::SUBSECTION_HEADING_GAP);
        ui.label(egui::RichText::new("matrix").strong());
        ui.add_space(spacing::SUBSECTION_HEADING_GAP);

        let matrix_configured = lib::config::matrix_channel_configured(config);
        if matrix_configured {
            if let Some(ref h) = config.channels.matrix.homeserver {
                dashboard::kv(ui, "homeserver", h.as_str());
            }
            let mode = if lib::config::resolve_matrix_access_token(config).is_some() {
                "access token"
            } else {
                "password login"
            };
            dashboard::kv(ui, "Mode", mode);
        } else {
            ui.label(egui::RichText::new("not configured").weak());
            ui.add_space(spacing::LINE);
        }

        ui.add_space(spacing::SUBSECTION_HEADING_GAP);
        ui.label(egui::RichText::new("signal").strong());
        ui.add_space(spacing::SUBSECTION_HEADING_GAP);

        let signal_configured = config
            .channels
            .signal
            .http_base
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if signal_configured {
            if let Some(ref h) = config.channels.signal.http_base {
                dashboard::kv(ui, "HTTP base", h.as_str());
            }
            if let Some(ref a) = config.channels.signal.account {
                dashboard::kv(ui, "Account", a.as_str());
            }
        } else {
            ui.label(egui::RichText::new("not configured").weak());
            ui.add_space(spacing::LINE);
        }

        ui.add_space(spacing::SUBSECTION_HEADING_GAP);
        ui.label(egui::RichText::new("telegram").strong());
        ui.add_space(spacing::SUBSECTION_HEADING_GAP);

        let telegram_configured = config.channels.telegram.bot_token.is_some()
            || config.channels.telegram.webhook_url.is_some();
        if telegram_configured {
            if let Some(ref t) = config.channels.telegram.bot_token {
                dashboard::kv(
                    ui,
                    "Bot token",
                    if t.trim().is_empty() {
                        "(empty)"
                    } else {
                        "set"
                    },
                );
            }
            if let Some(ref w) = config.channels.telegram.webhook_url {
                dashboard::kv(ui, "Webhook URL", w.as_str());
            }
        } else {
            ui.label(egui::RichText::new("not configured").weak());
            ui.add_space(spacing::LINE);
        }
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);

    dashboard::section_group(ui, "Providers", |ui| {
        if config.providers.entries.is_empty() {
            ui.label(egui::RichText::new("not configured").weak());
            return;
        }

        for def in &config.providers.entries {
            let resolved_base = lib::config::resolve_provider_base_url(&config.providers, &def.id);
            let resolved_key = lib::config::resolve_provider_api_key(&config.providers, &def.id);

            ui.add_space(spacing::SUBSECTION_HEADING_GAP);
            ui.label(egui::RichText::new(def.id.clone()).strong());
            ui.add_space(spacing::SUBSECTION_HEADING_GAP);
            dashboard::kv(ui, "Endpoint type", def.endpoint_type.as_str());
            if let Some(ref url) = resolved_base {
                dashboard::kv(ui, "Base URL", url.as_str());
            }
            let key_set = resolved_key
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            dashboard::kv(ui, "API key", if key_set { "(hidden)" } else { "(empty)" });
            if let Some(ref default_model) = def.default_model {
                if !default_model.trim().is_empty() {
                    dashboard::kv(ui, "Default model", default_model.trim());
                }
            }
            // Model discovery.
            let discovery_label = match def.model_discovery {
                lib::config::ModelDiscovery::Auto => "auto",
                lib::config::ModelDiscovery::Lmstudio => "lmstudio",
                lib::config::ModelDiscovery::Static => "static",
            };
            dashboard::kv(ui, "Model discovery", discovery_label);
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
        }
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);
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
        "(empty)".to_string()
    } else {
        v.join(", ")
    }
}

fn config_summary_right_column(
    ui: &mut egui::Ui,
    config: &lib::config::Config,
) {
    dashboard::section_group(ui, "Sandbox", |ui| {
        dashboard::kv(ui, "Mode", config.sandbox.mode.as_str());
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);

    dashboard::section_group(ui, "Agents", |ui| {
        for orch in &config.agents.orchestrators {
            let orch_id = orch.id.trim();
            let orch_id = if orch_id.is_empty() { "orchestrator" } else { orch_id };

            let orch_provider = orch
                .default_provider
                .as_deref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    config.providers.entries.first()
                        .map(|p| p.id.trim())
                        .unwrap_or("ollama")
                });
            let orch_model_fallback = lib::config::resolve_provider_default_model(&config.providers, orch_provider);
            let orch_model = orch
                .default_model
                .as_deref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or(orch_model_fallback.as_str());

            // Orchestrator subsection — title is just the lowercase agent id.
            ui.add_space(spacing::SUBSECTION_HEADING_GAP);
            ui.label(egui::RichText::new(orch_id).strong());
            ui.add_space(spacing::SUBSECTION_HEADING_GAP);
            dashboard::kv(ui, "Role", "orchestrator");
            dashboard::kv(ui, "Default provider", orch_provider);
            dashboard::kv(ui, "Default model", orch_model);
            let orch_ep = enabled_providers_display(&orch.enabled_providers);
            dashboard::kv(ui, "Enabled providers", orch_ep.as_str());
            let orch_skills = orch.enabled_skills_list();
            let orch_skills_csv = orch_skills.join(", ");
            dashboard::kv(
                ui,
                "Enabled skills",
                if orch_skills.is_empty() {
                    "(empty)"
                } else {
                    orch_skills_csv.as_str()
                },
            );
            match &orch.enabled_workers {
                None => dashboard::kv(ui, "Enabled workers", "(none)"),
                Some(workers) if workers.is_empty() => dashboard::kv(ui, "Enabled workers", "(all)"),
                Some(workers) => dashboard::kv(ui, "Enabled workers", workers.join(", ").as_str()),
            }
            let orch_mode = orch.context_mode();
            dashboard::kv(
                ui,
                "Context mode",
                match orch_mode {
                    lib::config::SkillContextMode::Full => "full",
                    lib::config::SkillContextMode::ReadOnDemand => "readOnDemand",
                },
            );

            // Orchestrator limit fields (same order as gateway screen).
            let mut any_limit = false;
            if let Some(n) = orch.max_tool_loops_per_turn {
                dashboard::kv(ui, "Max tool loops per turn", &n.to_string());
                any_limit = true;
            }
            if let Some(n) = orch.max_delegations_per_turn {
                dashboard::kv(ui, "Max delegations per turn", &n.to_string());
                any_limit = true;
            }
            if let Some(n) = orch.max_delegations_per_session {
                dashboard::kv(ui, "Max delegations per session", &n.to_string());
                any_limit = true;
            }
            if let Some(ref m) = orch.max_delegations_per_worker {
                if !m.is_empty() {
                    let display: Vec<String> = m
                        .iter()
                        .map(|(k, v)| format!("{} ({})", k, v))
                        .collect();
                    dashboard::kv(ui, "Max delegations per worker", display.join(", ").as_str());
                    any_limit = true;
                }
            }
            if any_limit {
                ui.add_space(spacing::LINE);
            }
        }

        if let Some(ref workers) = config.agents.workers {
            for w in workers {
                let wid = w.id.trim();
                if wid.is_empty() {
                    continue;
                }
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                // Worker subsection — title is just the lowercase worker id.
                ui.label(egui::RichText::new(wid).strong());
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                dashboard::kv(ui, "Role", "worker");
                let (wp, wm) = lib::orchestration::effective_worker_defaults(&config.providers, config.agents.default_orchestrator(), w);
                dashboard::kv(ui, "Default provider", wp.as_str());
                dashboard::kv(ui, "Default model", wm.as_str());
                let w_skills = lib::config::worker_enabled_skills_list(w);
                let w_skills_csv = w_skills.join(", ");
                dashboard::kv(
                    ui,
                    "Enabled skills",
                    if w_skills.is_empty() {
                        "(empty)"
                    } else {
                        w_skills_csv.as_str()
                    },
                );
                let w_mode = lib::config::worker_context_mode(w);
                dashboard::kv(
                    ui,
                    "Context mode",
                    match w_mode {
                        lib::config::SkillContextMode::Full => "full",
                        lib::config::SkillContextMode::ReadOnDemand => "readOnDemand",
                    },
                );
            }
        }
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);

    // Skills box on the right side, underneath the Agents box.
    dashboard::section_group(ui, "Skills", |ui| {
        let lock_mode = match config.skills.lock_mode {
            lib::config::SkillLockMode::Strict => "strict",
            lib::config::SkillLockMode::Warn => "warn",
        };
        dashboard::kv(ui, "Lock mode", lock_mode);
    });
}
