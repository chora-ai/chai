use eframe::egui;

use crate::app::ChaiApp;

pub fn ui_info_screen(app: &mut ChaiApp, ui: &mut egui::Ui, running: bool) {
    const INFO_LINE_SPACING: f32 = 6.0;
    const INFO_SUBSECTION_SPACING: f32 = 18.0;

    crate::app::ui_screen(
        ui,
        "Info",
        Some("Values below are loaded from the running gateway."),
        |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Gateway
                ui.label(egui::RichText::new("Gateway").strong());
                ui.add_space(INFO_LINE_SPACING);
                ui.label(format!(
                    "Status: {}",
                    if running { "running" } else { "stopped" }
                ));
                ui.add_space(INFO_LINE_SPACING);
                if let Some(ref s) = app.gateway_status {
                    ui.label(format!("Bind: {}", s.bind));
                    ui.add_space(INFO_LINE_SPACING);
                    ui.label(format!("Port: {}", s.port));
                    ui.add_space(INFO_LINE_SPACING);
                    ui.label(format!("Protocol: {}", s.protocol));
                    ui.add_space(INFO_LINE_SPACING);
                    ui.label(format!("Auth: {}", s.auth));
                    ui.add_space(INFO_LINE_SPACING);
                } else if running {
                    ui.label("(waiting for status from gateway)");
                    ui.add_space(INFO_LINE_SPACING);
                } else {
                    ui.label("(start gateway to see info)");
                    ui.add_space(INFO_LINE_SPACING);
                }
                if let Some(ref err) = app.gateway_error {
                    ui.colored_label(egui::Color32::RED, err);
                    ui.add_space(INFO_LINE_SPACING);
                }
                ui.add_space(INFO_SUBSECTION_SPACING);

                // Models
                ui.label(egui::RichText::new("Models").strong());
                ui.add_space(INFO_LINE_SPACING);
                if let Some(ref s) = app.gateway_status {
                    let backend = s.default_backend.as_deref().unwrap_or("ollama");
                    let model = app
                        .current_model
                        .clone()
                        .or_else(|| s.default_model.clone())
                        .unwrap_or_else(|| "—".to_string());
                    ui.label(format!("Backend: {}", backend));
                    ui.add_space(INFO_LINE_SPACING);
                    ui.label(format!("Model: {}", model));
                    ui.add_space(INFO_LINE_SPACING);
                    if !s.ollama_models.is_empty() {
                        ui.label(format!("Ollama models: {}", s.ollama_models.join(", ")));
                        ui.add_space(INFO_LINE_SPACING);
                    }
                    if !s.lm_studio_models.is_empty() {
                        ui.label(format!(
                            "LM Studio models: {}",
                            s.lm_studio_models.join(", ")
                        ));
                        ui.add_space(INFO_LINE_SPACING);
                    }
                } else if running {
                    ui.label("(waiting for status from gateway)");
                    ui.add_space(INFO_LINE_SPACING);
                } else {
                    ui.label("(start gateway to see info)");
                    ui.add_space(INFO_LINE_SPACING);
                }
                ui.add_space(INFO_SUBSECTION_SPACING);

                // Context
                ui.label(egui::RichText::new("Context").strong());
                ui.add_space(INFO_LINE_SPACING);
                if let Some(ref s) = app.gateway_status {
                    if let Some(mode) = s.context_mode.as_deref() {
                        ui.label(format!("Context mode: {}", mode));
                        ui.add_space(INFO_LINE_SPACING);
                    } else {
                        ui.label("Context mode: (not reported)");
                        ui.add_space(INFO_LINE_SPACING);
                    }
                } else if running {
                    ui.label("(waiting for status from gateway)");
                    ui.add_space(INFO_LINE_SPACING);
                } else {
                    ui.label("(start gateway to see info)");
                    ui.add_space(INFO_LINE_SPACING);
                }
            });
        },
    );
}

