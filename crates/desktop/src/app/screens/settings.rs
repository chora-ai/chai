use eframe::egui;

use crate::app::ui::{dashboard, readonly_code, spacing, view_toggle};
use crate::app::{ChaiApp, SettingsViewMode};

pub fn ui_settings_screen(app: &mut ChaiApp, ui: &mut egui::Ui) {
    let desktop_config_load_error;
    let desktop_config = match app.load_desktop_config_cached() {
        Ok(config) => {
            desktop_config_load_error = None;
            config.clone()
        }
        Err(e) => {
            log::warn!("failed to load desktop.json, using defaults: {}", e);
            desktop_config_load_error = Some(e.to_string());
            lib::config::DesktopConfig::default()
        }
    };

    let config_path = lib::profile::chai_home()
        .ok()
        .map(|h| lib::config::DesktopConfig::path(&h));

    let subtitle = if let Some(ref path) = config_path {
        if !path.as_os_str().is_empty() {
            Some(format!("Values loaded from {}", path.display()))
        } else {
            None
        }
    } else {
        None
    };

    crate::app::ui_screen(ui, "Settings", subtitle.as_deref(), |ui| {
        view_toggle::settings_view_radios(ui, &mut app.settings_view_mode);
        ui.add_space(spacing::SUBSECTION);

        egui::ScrollArea::vertical()
            .max_height(ui.available_height().max(80.0))
            .show(ui, |ui| {
                if app.settings_view_mode == SettingsViewMode::RawJson {
                    let Some(ref path) = config_path else {
                        ui.label(egui::RichText::new("(could not resolve chai home)").weak());
                        return;
                    };
                    if path.as_os_str().is_empty() {
                        ui.label(egui::RichText::new("(no settings path resolved)").weak());
                        return;
                    }
                    if !path.exists() {
                        app.settings_raw_display_buffer.clear();
                        ui.label(
                            egui::RichText::new(format!(
                                "No file at {} — the Dashboard view shows defaults.",
                                path.display()
                            )),
                        );
                        return;
                    }
                    match std::fs::read_to_string(path) {
                        Ok(s) => {
                            if app.settings_raw_display_buffer.as_str() != s.as_str() {
                                app.settings_raw_display_buffer = s;
                            }
                            readonly_code::read_only_code_block(
                                ui,
                                "settings_raw_textedit",
                                &mut app.settings_raw_display_buffer,
                                28,
                            );
                        }
                        Err(e) => {
                            ui.colored_label(
                                egui::Color32::RED,
                                format!("failed to read {}: {}", path.display(), e),
                            );
                        }
                    }
                    return;
                }

                if let Some(ref err) = desktop_config_load_error {
                    ui.colored_label(
                        egui::Color32::from_rgb(200, 150, 50),
                        format!("Using default settings (failed to load desktop.json: {})", err),
                    );
                    ui.add_space(spacing::SUBSECTION);
                }
                settings_summary_dashboard(ui, &desktop_config);
            });
    });
}

fn settings_summary_dashboard(ui: &mut egui::Ui, config: &lib::config::DesktopConfig) {
    dashboard::dashboard_two_columns(ui, |left, _right| {
        left.vertical(|ui| {
            settings_left_column(ui, config);
        });
    });
}

fn settings_left_column(ui: &mut egui::Ui, config: &lib::config::DesktopConfig) {
    dashboard::section_group(ui, "Appearance", |ui| {
        dashboard::kv(ui, "Theme", &config.appearance.theme);
        dashboard::kv(ui, "Font size", &format!("{}pt", config.appearance.font_size));
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);

    dashboard::section_group(ui, "Logs", |ui| {
        dashboard::kv(ui, "Buffer size", &config.logs.buffer_size.to_string());
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);
}
