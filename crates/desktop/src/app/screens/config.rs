use eframe::egui;

use crate::app::ChaiApp;

pub fn ui_config_screen(app: &mut ChaiApp, ui: &mut egui::Ui) {
    const INFO_LINE_SPACING: f32 = 6.0;
    const INFO_SUBSECTION_SPACING: f32 = 18.0;
    app.invalidate_enabled_providers_cache();
    let (config, config_path) = lib::config::load_config(None)
        .unwrap_or((lib::config::Config::default(), std::path::PathBuf::new()));
    if app.default_model.is_none() {
        let (_, model) = lib::config::resolve_effective_provider_and_model(&config.agents);
        app.default_model = Some(model);
    }

    let subtitle = if !config_path.as_os_str().is_empty() {
        Some(format!(
            "Values below are loaded from: {}",
            config_path.display()
        ))
    } else {
        None
    };

    crate::app::ui_screen(ui, "Config", subtitle.as_deref(), |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(egui::RichText::new("Gateway").strong());
            ui.add_space(INFO_LINE_SPACING);
            ui.label(format!("Bind: {}", config.gateway.bind.trim()));
            ui.add_space(INFO_LINE_SPACING);
            ui.label(format!("Port: {}", config.gateway.port));
            ui.add_space(INFO_LINE_SPACING);
            let auth_mode = match config.gateway.auth.mode {
                lib::config::GatewayAuthMode::None => "none",
                lib::config::GatewayAuthMode::Token => "token",
            };
            ui.label(format!("Auth: {}", auth_mode));
            ui.add_space(INFO_LINE_SPACING);
            ui.add_space(INFO_SUBSECTION_SPACING);

            ui.label(egui::RichText::new("Channels").strong());
            ui.add_space(INFO_LINE_SPACING);
            let telegram_configured = config.channels.telegram.bot_token.is_some()
                || config.channels.telegram.webhook_url.is_some();
            if telegram_configured {
                if let Some(ref t) = config.channels.telegram.bot_token {
                    ui.label(format!(
                        "Telegram bot token: {}",
                        if t.trim().is_empty() { "(empty)" } else { "set" }
                    ));
                    ui.add_space(INFO_LINE_SPACING);
                }
                if let Some(ref w) = config.channels.telegram.webhook_url {
                    ui.label(format!("Telegram webhook: {}", w));
                    ui.add_space(INFO_LINE_SPACING);
                }
            } else {
                ui.label("Telegram: not configured.");
                ui.add_space(INFO_LINE_SPACING);
            }
            let matrix_configured = config.channels.matrix.homeserver.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)
                && (config.channels.matrix.access_token.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)
                    || (config.channels.matrix.user.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)
                        && config.channels.matrix.password.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)));
            if matrix_configured {
                if let Some(ref h) = config.channels.matrix.homeserver {
                    ui.label(format!("Matrix homeserver: {}", h));
                    ui.add_space(INFO_LINE_SPACING);
                }
                ui.label(format!(
                    "Matrix: {}",
                    if config
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
                    }
                ));
                ui.add_space(INFO_LINE_SPACING);
            } else {
                ui.label("Matrix: not configured.");
                ui.add_space(INFO_LINE_SPACING);
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
                    ui.label(format!("Signal (signal-cli HTTP): {}", h));
                    ui.add_space(INFO_LINE_SPACING);
                }
                if let Some(ref a) = config.channels.signal.account {
                    ui.label(format!("Signal account: {}", a));
                    ui.add_space(INFO_LINE_SPACING);
                }
            } else {
                ui.label("Signal: not configured.");
                ui.add_space(INFO_LINE_SPACING);
            }
            ui.add_space(INFO_SUBSECTION_SPACING);

            ui.label(egui::RichText::new("Agents").strong());
            ui.add_space(INFO_LINE_SPACING);
            let (default_provider, default_model) =
                lib::config::resolve_effective_provider_and_model(&config.agents);
            ui.label(format!("Default provider: {}", default_provider));
            ui.add_space(INFO_LINE_SPACING);
            ui.label(format!("Default model: {}", default_model));
            ui.add_space(INFO_LINE_SPACING);
            if let Some(ref oid) = config.agents.orchestrator_id {
                ui.label(format!("Orchestrator id: {}", oid));
                ui.add_space(INFO_LINE_SPACING);
            }
            let enabled_providers_display =
                if config.agents.enabled_providers.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
                    default_provider
                } else {
                    let v = config.agents.enabled_providers.as_ref().unwrap();
                    let s = v
                        .iter()
                        .map(|s| s.as_str())
                        .filter(|s| !s.trim().is_empty())
                        .collect::<Vec<_>>()
                        .join(", ");
                    if s.is_empty() {
                        default_provider
                    } else {
                        s
                    }
                };
            ui.label(format!("Enabled providers: {}", enabled_providers_display));
            ui.add_space(INFO_LINE_SPACING);
            if let Some(ref w) = config.agents.workspace {
                ui.label(format!("Workspace: {}", w.display()));
                ui.add_space(INFO_LINE_SPACING);
            }
            if let Some(ref b) = config.providers {
                if b.ollama
                    .as_ref()
                    .and_then(|o| o.base_url.as_ref())
                    .map(|u| !u.trim().is_empty())
                    .unwrap_or(false)
                {
                    ui.label(format!(
                        "Ollama base URL: {}",
                        b.ollama.as_ref().unwrap().base_url.as_ref().unwrap()
                    ));
                    ui.add_space(INFO_LINE_SPACING);
                }
                if b.lms
                    .as_ref()
                    .and_then(|l| l.base_url.as_ref())
                    .map(|u| !u.trim().is_empty())
                    .unwrap_or(false)
                {
                    ui.label(format!(
                        "LM Studio base URL: {}",
                        b.lms.as_ref().unwrap().base_url.as_ref().unwrap()
                    ));
                    ui.add_space(INFO_LINE_SPACING);
                }
            }
            ui.add_space(INFO_SUBSECTION_SPACING);

            ui.label(egui::RichText::new("Skills").strong());
            ui.add_space(INFO_LINE_SPACING);
            let skills_configured = config.skills.directory.is_some()
                || !config.skills.extra_dirs.is_empty()
                || !config.skills.enabled.is_empty()
                || config.skills.context_mode != lib::config::SkillContextMode::Full;
            if skills_configured {
                if let Some(ref d) = config.skills.directory {
                    ui.label(format!("Directory: {}", d.display()));
                    ui.add_space(INFO_LINE_SPACING);
                }
                if !config.skills.extra_dirs.is_empty() {
                    ui.label(format!(
                        "Extra dirs: {}",
                        config
                            .skills
                            .extra_dirs
                            .iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    ui.add_space(INFO_LINE_SPACING);
                }
                if !config.skills.enabled.is_empty() {
                    ui.label(format!("Enabled: {}", config.skills.enabled.join(", ")));
                    ui.add_space(INFO_LINE_SPACING);
                }
                let context_mode_str = match config.skills.context_mode {
                    lib::config::SkillContextMode::Full => "full",
                    lib::config::SkillContextMode::ReadOnDemand => "readOnDemand",
                };
                ui.label(format!("Context mode: {}", context_mode_str));
                ui.add_space(INFO_LINE_SPACING);
            } else {
                ui.label("Not configured.");
                ui.add_space(INFO_LINE_SPACING);
            }
        });
    });
}

