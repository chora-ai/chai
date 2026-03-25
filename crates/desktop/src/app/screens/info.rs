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
                let enabled_list = app.enabled_providers();
                let available_providers = if enabled_list.is_empty() {
                    "—".to_string()
                } else {
                    enabled_list.join(", ")
                };
                let provider_enabled =
                    |id: &str| enabled_list.iter().any(|e| e == id);
                if let Some(ref s) = app.gateway_status {
                    let provider = app
                        .current_provider
                        .as_deref()
                        .or(s.default_provider.as_deref())
                        .unwrap_or("ollama");
                    let model = app
                        .current_model
                        .clone()
                        .or_else(|| s.default_model.clone())
                        .unwrap_or_else(|| "—".to_string());
                    ui.label(format!("Current provider: {}", provider));
                    ui.add_space(INFO_LINE_SPACING);
                    ui.label(format!("Current model: {}", model));
                    ui.add_space(INFO_LINE_SPACING);
                    ui.label(format!("Available providers: {}", available_providers));
                    ui.add_space(INFO_LINE_SPACING);

                    if provider_enabled("ollama") {
                        let ollama_models = if s.ollama_models.is_empty() {
                            "(no models discovered)".to_string()
                        } else {
                            s.ollama_models.join(", ")
                        };
                        ui.label(format!("Ollama models: {}", ollama_models));
                        ui.add_space(INFO_LINE_SPACING);
                    }
                    if provider_enabled("lms") {
                        let lms_models = if s.lms_models.is_empty() {
                            "(no models discovered)".to_string()
                        } else {
                            s.lms_models.join(", ")
                        };
                        ui.label(format!("LM Studio models: {}", lms_models));
                        ui.add_space(INFO_LINE_SPACING);
                    }
                    if provider_enabled("vllm") {
                        let vllm_models = if s.vllm_models.is_empty() {
                            "(no models discovered)".to_string()
                        } else {
                            s.vllm_models.join(", ")
                        };
                        ui.label(format!("vLLM models: {}", vllm_models));
                        ui.add_space(INFO_LINE_SPACING);
                    }
                    if provider_enabled("nim") {
                        let nim_models = if s.nim_models.is_empty() {
                            "(static catalog not loaded)".to_string()
                        } else {
                            s.nim_models.join(", ")
                        };
                        ui.label(format!("NIM models: {}", nim_models));
                        ui.add_space(INFO_LINE_SPACING);
                    }
                    if provider_enabled("openai") {
                        let openai_models = if s.openai_models.is_empty() {
                            "(no models discovered)".to_string()
                        } else {
                            s.openai_models.join(", ")
                        };
                        ui.label(format!("OpenAI models: {}", openai_models));
                        ui.add_space(INFO_LINE_SPACING);
                    }
                    if provider_enabled("hf") {
                        let hf_models = if s.hf_models.is_empty() {
                            "(no models discovered)".to_string()
                        } else {
                            s.hf_models.join(", ")
                        };
                        ui.label(format!("Hugging Face models: {}", hf_models));
                        ui.add_space(INFO_LINE_SPACING);
                    }

                    let cat_rows: Vec<_> = s
                        .orchestration_catalog
                        .iter()
                        .filter(|row| provider_enabled(row.provider.as_str()))
                        .collect();
                    let n_cat = cat_rows.len();
                    if n_cat > 0 {
                        ui.add_space(INFO_SUBSECTION_SPACING);
                        ui.label(egui::RichText::new("Orchestration catalog").strong());
                        ui.add_space(INFO_LINE_SPACING);
                        egui::CollapsingHeader::new(format!(
                            "{} entries (merged discovery + allowlist)",
                            n_cat
                        ))
                        .default_open(false)
                        .show(ui, |ui| {
                            for row in cat_rows {
                                let mut bits: Vec<String> = Vec::new();
                                bits.push(format!("{} / {}", row.provider, row.model));
                                if !row.discovered {
                                    bits.push("not in discovery".to_string());
                                }
                                if let Some(l) = row.local {
                                    bits.push(format!("local={}", l));
                                }
                                if let Some(t) = row.tool_capable {
                                    bits.push(format!("toolCapable={}", t));
                                }
                                ui.label(
                                    egui::RichText::new(bits.join(" · "))
                                        .small()
                                        .weak(),
                                );
                            }
                        });
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

