use eframe::egui;

use crate::app::ChaiApp;

pub fn ui_config_screen(app: &mut ChaiApp, ui: &mut egui::Ui) {
    const INFO_LINE_SPACING: f32 = 6.0;
    const INFO_SUBSECTION_SPACING: f32 = 18.0;
    ui.add_space(24.0);
    ui.heading("Config");
    ui.add_space(ChaiApp::SCREEN_TITLE_BOTTOM_SPACING);
    let (config, config_path) = lib::config::load_config(None)
        .unwrap_or((lib::config::Config::default(), std::path::PathBuf::new()));
    if app.default_model.is_none() {
        let (_, model) = lib::config::resolve_effective_backend_and_model(&config.agents);
        app.default_model = Some(model);
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        if !config_path.as_os_str().is_empty() {
            ui.label(format!("Values below are loaded from: {}", config_path.display()));
            ui.add_space(INFO_LINE_SPACING);
        }
        ui.add_space(INFO_SUBSECTION_SPACING);

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
        ui.add_space(INFO_SUBSECTION_SPACING);

        ui.label(egui::RichText::new("Agents").strong());
        ui.add_space(INFO_LINE_SPACING);
        let (default_backend, default_model) =
            lib::config::resolve_effective_backend_and_model(&config.agents);
        ui.label(format!("Default backend: {}", default_backend));
        ui.add_space(INFO_LINE_SPACING);
        ui.label(format!("Default model: {}", default_model));
        ui.add_space(INFO_LINE_SPACING);
        let enabled_backends_display =
            if config.agents.enabled_backends.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
                default_backend
            } else {
                let v = config.agents.enabled_backends.as_ref().unwrap();
                let s = v
                    .iter()
                    .map(|s| s.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join(", ");
                if s.is_empty() {
                    default_backend
                } else {
                    s
                }
            };
        ui.label(format!("Enabled backends: {}", enabled_backends_display));
        ui.add_space(INFO_LINE_SPACING);
        if let Some(ref w) = config.agents.workspace {
            ui.label(format!("Workspace: {}", w.display()));
            ui.add_space(INFO_LINE_SPACING);
        }
        if let Some(ref b) = config.agents.backends {
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
            if b.lm_studio
                .as_ref()
                .and_then(|l| l.base_url.as_ref())
                .map(|u| !u.trim().is_empty())
                .unwrap_or(false)
            {
                ui.label(format!(
                    "LM Studio base URL: {}",
                    b.lm_studio.as_ref().unwrap().base_url.as_ref().unwrap()
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
            || config.skills.context_mode != lib::config::SkillContextMode::Full
            || config.skills.allow_scripts;
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
            if config.skills.allow_scripts {
                ui.label("Allow scripts: true");
                ui.add_space(INFO_LINE_SPACING);
            }
        } else {
            ui.label("Not configured.");
            ui.add_space(INFO_LINE_SPACING);
        }

        ui.add_space(ChaiApp::SCREEN_FOOTER_SPACING);
    });
}

