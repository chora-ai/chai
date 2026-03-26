use eframe::egui;

use crate::app::ui::{dashboard, readonly_code, spacing, view_toggle};
use crate::app::{ChaiApp, GatewayStatusDetails, StatusViewMode};

pub fn ui_status_screen(app: &mut ChaiApp, ui: &mut egui::Ui, running: bool) {
    crate::app::ui_screen(
        ui,
        "Status",
        Some(if running {
            "Values below are loaded from the gateway status."
        } else {
            "Start the gateway to load the gateway status."
        }),
        |ui| {
            if running {
                view_toggle::status_view_radios(ui, &mut app.status_view_mode);
                ui.add_space(spacing::SUBSECTION);
            }

            egui::ScrollArea::vertical()
                .max_height(ui.available_height().max(80.0))
                .show(ui, |ui| {
                    if !running {
                        return;
                    }

                    if app.status_view_mode == StatusViewMode::RawJson {
                        if let Some(ref s) = app.gateway_status {
                            if let Some(ref j) = s.status_response_json {
                                let mut buf = j.clone();
                                readonly_code::read_only_code_block(
                                    ui,
                                    "status_raw_json",
                                    &mut buf,
                                    28,
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("(raw JSON not available for this status)")
                                        .weak(),
                                );
                            }
                        } else {
                            ui.label("Loading from gateway status...");
                        }
                        return;
                    }

                    status_summary_dashboard(
                        ui,
                        app.gateway_status.as_ref(),
                        app.gateway_error.as_deref(),
                    );
                });
        },
    );
}

fn status_summary_dashboard(
    ui: &mut egui::Ui,
    status: Option<&GatewayStatusDetails>,
    gateway_error: Option<&str>,
) {
    dashboard::dashboard_two_columns(ui, |left, right| {
        left.vertical(|ui| {
            status_column_gateway_agents(ui, status, gateway_error);
        });
        right.vertical(|ui| {
            status_column_models_context(ui, status);
        });
    });
}

fn status_column_gateway_agents(
    ui: &mut egui::Ui,
    status: Option<&GatewayStatusDetails>,
    gateway_error: Option<&str>,
) {
    dashboard::section_group(ui, "Gateway", |ui| {
        if let Some(s) = status {
            dashboard::kv(ui, "Status", "running");
            dashboard::kv(ui, "Bind", s.bind.trim());
            dashboard::kv(ui, "Port", &s.port.to_string());
            dashboard::kv(ui, "Protocol", &s.protocol.to_string());
            dashboard::kv(ui, "Auth", s.auth.trim());
        } else {
            ui.label(egui::RichText::new("Loading from gateway status...").weak());
        }
        if let Some(err) = gateway_error {
            ui.colored_label(egui::Color32::RED, err);
        }
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);

    dashboard::section_group(ui, "Agents", |ui| {
        if let Some(s) = status {
            ui.add_space(spacing::SUBSECTION_HEADING_GAP);
            ui.label(egui::RichText::new("Orchestrator").strong());
            ui.add_space(spacing::SUBSECTION_HEADING_GAP);
            let oid = s
                .orchestrator_id
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .unwrap_or("orchestrator");
            dashboard::kv(ui, "id", oid);
            let dp = s.default_provider.as_deref().unwrap_or("—");
            let dm = s.default_model.as_deref().unwrap_or("—");
            dashboard::kv(ui, "default provider", dp);
            dashboard::kv(ui, "default model", dm);

            let cat_rows: Vec<_> = s.orchestration_catalog.iter().collect();
            let n_cat = cat_rows.len();

            ui.add_space(spacing::TABLE_BLOCK_AFTER);
            ui.add_space(spacing::SUBSECTION_HEADING_GAP);
            ui.label(egui::RichText::new("Workers").strong());
            ui.add_space(spacing::SUBSECTION_HEADING_GAP);
            if s.workers.is_empty() {
                ui.label(egui::RichText::new("none configured").weak());
                if n_cat > 0 {
                    ui.add_space(spacing::TABLE_BLOCK_AFTER);
                }
            } else {
                egui::Grid::new("status_workers_grid")
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
                        for w in &s.workers {
                            dashboard::grid_cell(ui, |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(w.id.trim())));
                            });
                            let wp = w.default_provider.trim();
                            let wm = w.default_model.trim();
                            dashboard::grid_cell(ui, |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(if wp.is_empty() {
                                    "—"
                                } else {
                                    wp
                                })));
                            });
                            dashboard::grid_cell(ui, |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(if wm.is_empty() {
                                    "—"
                                } else {
                                    wm
                                })));
                            });
                            ui.end_row();
                        }
                    });
                ui.add_space(spacing::TABLE_BLOCK_AFTER);
            }

            if n_cat > 0 {
                ui.add_space(spacing::COLLAPSING_HEADER_BEFORE);
                egui::CollapsingHeader::new(format!(
                    "Orchestration catalog — {} entries (merged discovery + allowlist)",
                    n_cat
                ))
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(spacing::COLLAPSING_BODY_INSET);
                    egui::Grid::new("status_catalog_grid")
                        .num_columns(3)
                        .striped(true)
                        .spacing([spacing::GRID_CELL_SPACING, spacing::GRID_CELL_SPACING])
                        .show(ui, |ui| {
                            dashboard::grid_cell(ui, |ui| {
                                ui.label(dashboard::grid_header_rich(ui, "Provider"));
                            });
                            dashboard::grid_cell(ui, |ui| {
                                ui.label(dashboard::grid_header_rich(ui, "Model"));
                            });
                            dashboard::grid_cell(ui, |ui| {
                                ui.label(dashboard::grid_header_rich(ui, "Flags"));
                            });
                            ui.end_row();
                            for row in &cat_rows {
                                dashboard::grid_cell(ui, |ui| {
                                    ui.add(egui::Label::new(egui::RichText::new(&row.provider)));
                                });
                                dashboard::grid_cell(ui, |ui| {
                                    ui.add(egui::Label::new(egui::RichText::new(&row.model)));
                                });
                                let mut bits: Vec<String> = Vec::new();
                                if !row.discovered {
                                    bits.push("not in discovery".to_string());
                                }
                                if let Some(l) = row.local {
                                    bits.push(format!("local={}", l));
                                }
                                if let Some(t) = row.tool_capable {
                                    bits.push(format!("toolCapable={}", t));
                                }
                                dashboard::grid_cell(ui, |ui| {
                                    ui.add(egui::Label::new(
                                        egui::RichText::new(bits.join(" · ")).weak(),
                                    ));
                                });
                                ui.end_row();
                            }
                        });
                    ui.add_space(spacing::COLLAPSING_BODY_INSET);
                });
                ui.add_space(spacing::TABLE_BLOCK_AFTER);
            }
        } else {
            ui.label(egui::RichText::new("Loading from gateway status...").weak());
        }
    });
}

fn status_column_models_context(ui: &mut egui::Ui, status: Option<&GatewayStatusDetails>) {
    dashboard::section_group(ui, "Context", |ui| {
        if let Some(s) = status {
            if let Some(ref d) = s.date {
                let t = d.trim();
                if !t.is_empty() {
                    dashboard::kv(ui, "Today's date (sent to model)", t);
                }
            }
            if let Some(mode) = s.context_mode.as_deref() {
                dashboard::kv(ui, "Context mode", mode);
            } else {
                dashboard::kv(ui, "Context mode", "(not reported)");
            }
        } else {
            ui.label(egui::RichText::new("Loading from gateway status...").weak());
        }
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);

    dashboard::section_group(ui, "Models", |ui| {
        if let Some(s) = status {
            let show = |id: &str| {
                s.enabled_providers
                    .as_ref()
                    .map(|ids| ids.iter().any(|x| x == id))
                    .unwrap_or(true)
            };

            if show("ollama") {
                backend_models_wrapped(
                    ui,
                    "Ollama",
                    &s.ollama_models,
                    "(no models discovered)",
                );
            }
            if show("lms") {
                backend_models_wrapped(
                    ui,
                    "LM Studio",
                    &s.lms_models,
                    "(no models discovered)",
                );
            }
            if show("vllm") {
                backend_models_wrapped(
                    ui,
                    "vLLM",
                    &s.vllm_models,
                    "(no models discovered)",
                );
            }
            if show("nim") {
                backend_models_wrapped(
                    ui,
                    "NIM",
                    &s.nim_models,
                    "(static catalog not loaded)",
                );
            }
            if show("openai") {
                backend_models_wrapped(
                    ui,
                    "OpenAI",
                    &s.openai_models,
                    "(no models discovered)",
                );
            }
            if show("hf") {
                backend_models_wrapped(
                    ui,
                    "Hugging Face",
                    &s.hf_models,
                    "(no models discovered)",
                );
            }
            if s.enabled_providers.as_ref().map(|v| v.is_empty()).unwrap_or(false) {
                ui.label(
                    egui::RichText::new("no backends enabled for provider discovery").weak(),
                );
                ui.add_space(spacing::LINE);
            }
        } else {
            ui.label(egui::RichText::new("Loading from gateway status...").weak());
        }
    });
}

fn backend_models_wrapped(
    ui: &mut egui::Ui,
    title: &str,
    models: &[String],
    empty_label: &str,
) {
    ui.add_space(spacing::SUBSECTION_HEADING_GAP);
    ui.label(egui::RichText::new(title).strong());
    ui.add_space(spacing::SUBSECTION_HEADING_GAP);

    if models.is_empty() {
        ui.label(egui::RichText::new(empty_label).weak());
        ui.add_space(spacing::TABLE_BLOCK_AFTER);
        return;
    }

    egui::Grid::new(format!("status_models_grid_{}", title))
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
            for m in models {
                dashboard::grid_cell(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(title)));
                });
                dashboard::grid_cell(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(m)));
                });
                ui.end_row();
            }
        });

    ui.add_space(spacing::TABLE_BLOCK_AFTER);
}
