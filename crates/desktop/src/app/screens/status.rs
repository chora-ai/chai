use eframe::egui;
use serde_json::Value as JsonValue;

use crate::app::ui::{dashboard, readonly_code, spacing, view_toggle};
use crate::app::{ChaiApp, GatewayStatusDetails, StatusViewMode};

/// Order matches gateway **`status`** payload **`providers`** object (see **`server.rs`**).
const STATUS_PROVIDER_IDS: &[&str] =
    &["ollama", "lms", "vllm", "nim", "openai", "hf"];

const PROVIDER_DISPLAY: &[(&str, &str)] = &[
    ("ollama", "Ollama"),
    ("lms", "LM Studio"),
    ("vllm", "vLLM"),
    ("nim", "NIM"),
    ("openai", "OpenAI"),
    ("hf", "Hugging Face"),
];

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
            status_column_left(ui, status, gateway_error);
        });
        right.vertical(|ui| {
            status_column_agents(ui, status);
            ui.add_space(spacing::DASHBOARD_COLUMN_GAP);
            status_skill_packages_section(ui, status);
        });
    });
}

fn truncate_display_chars(s: &str, max_chars: usize) -> String {
    let mut it = s.chars();
    let prefix: String = it.by_ref().take(max_chars).collect();
    if it.next().is_some() {
        format!("{}…", prefix)
    } else {
        prefix
    }
}

fn channel_status_field_display(v: &JsonValue, max_chars: usize) -> String {
    let s = match v {
        JsonValue::String(x) => x.clone(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Null => "—".to_string(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::Array(a) => {
            if a.is_empty() {
                "[]".to_string()
            } else if a.len() <= 3 {
                serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string())
            } else {
                format!("[{} items]", a.len())
            }
        }
        JsonValue::Object(_) => serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string()),
    };
    truncate_display_chars(&s, max_chars)
}

fn status_clock_section(ui: &mut egui::Ui, status: Option<&GatewayStatusDetails>) {
    dashboard::section_group(ui, "Clock", |ui| {
        let Some(s) = status else {
            ui.label(egui::RichText::new("Loading from gateway status...").weak());
            return;
        };
        if let Some(ref d) = s.date {
            let t = d.trim();
            if !t.is_empty() {
                dashboard::kv(ui, "date", t);
                return;
            }
        }
        ui.label(egui::RichText::new("(not reported)").weak());
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);
}

fn status_gateway_section(ui: &mut egui::Ui, status: Option<&GatewayStatusDetails>, gateway_error: Option<&str>) {
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
}

fn status_channels_section(ui: &mut egui::Ui, status: Option<&GatewayStatusDetails>) {
    dashboard::section_group(ui, "Channels", |ui| {
        let Some(s) = status else {
            ui.label(egui::RichText::new("Loading from gateway status...").weak());
            return;
        };
        let Some(ch) = s.channels_block.as_ref() else {
            ui.label(egui::RichText::new("(no channels block)").weak());
            return;
        };
        let Some(obj) = ch.as_object() else {
            ui.label(egui::RichText::new("(invalid channels)").weak());
            return;
        };
        if obj.is_empty() {
            ui.label(egui::RichText::new("(empty)").weak());
            return;
        }
        let mut names: Vec<_> = obj.keys().cloned().collect();
        names.sort();
        for name in names {
            let Some(per) = obj.get(&name) else {
                continue;
            };
            ui.add_space(spacing::SUBSECTION_HEADING_GAP);
            ui.label(egui::RichText::new(&name).strong());
            ui.add_space(spacing::SUBSECTION_HEADING_GAP);
            let Some(po) = per.as_object() else {
                dashboard::kv(ui, "value", &channel_status_field_display(per, 120));
                continue;
            };
            let mut keys: Vec<_> = po.keys().cloned().collect();
            keys.sort();
            for k in keys {
                let Some(val) = po.get(&k) else {
                    continue;
                };
                dashboard::kv(ui, k.as_str(), &channel_status_field_display(val, 160));
            }
        }
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);
}

fn models_from_provider_json(v: &JsonValue) -> Vec<String> {
    v.get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|o| {
                    o.get("name")
                        .and_then(|n| n.as_str())
                        .map(std::string::ToString::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn status_providers_section(ui: &mut egui::Ui, status: Option<&GatewayStatusDetails>) {
    dashboard::section_group(ui, "Providers", |ui| {
        let Some(s) = status else {
            ui.label(egui::RichText::new("Loading from gateway status...").weak());
            return;
        };

        if let Some(ref block) = s.providers_block {
            if let Some(obj) = block.as_object() {
                for id in STATUS_PROVIDER_IDS {
                    let Some(per) = obj.get(*id) else {
                        continue;
                    };
                    let title = PROVIDER_DISPLAY
                        .iter()
                        .find(|(k, _)| *k == *id)
                        .map(|(_, d)| *d)
                        .unwrap_or(id);
                    let discovery = per
                        .get("discovery")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let models = models_from_provider_json(per);

                    ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                    ui.label(egui::RichText::new(title).strong());
                    ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                    dashboard::kv(
                        ui,
                        "Discovery",
                        if discovery { "on" } else { "off" },
                    );

                    if models.is_empty() {
                        let empty_label = if *id == "nim" {
                            "(static catalog not loaded)"
                        } else {
                            "(no models discovered)"
                        };
                        ui.label(egui::RichText::new(empty_label).weak());
                    } else {
                        egui::Grid::new(format!("status_providers_models_{id}"))
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
                                for m in &models {
                                    dashboard::grid_cell(ui, |ui| {
                                        ui.add(egui::Label::new(egui::RichText::new(title)));
                                    });
                                    dashboard::grid_cell(ui, |ui| {
                                        ui.add(egui::Label::new(egui::RichText::new(m)));
                                    });
                                    ui.end_row();
                                }
                            });
                    }
                    ui.add_space(spacing::TABLE_BLOCK_AFTER);
                }
                return;
            }
        }

        // Fallback when **`providers`** block missing (older gateway).
        for (id, title, models, empty_label) in [
            ("ollama", "Ollama", &s.ollama_models[..], "(no models discovered)"),
            ("lms", "LM Studio", &s.lms_models[..], "(no models discovered)"),
            ("vllm", "vLLM", &s.vllm_models[..], "(no models discovered)"),
            ("nim", "NIM", &s.nim_models[..], "(static catalog not loaded)"),
            ("openai", "OpenAI", &s.openai_models[..], "(no models discovered)"),
            ("hf", "Hugging Face", &s.hf_models[..], "(no models discovered)"),
        ] {
            if s.enabled_providers
                .as_ref()
                .map(|ids| ids.iter().any(|x| x == id))
                .unwrap_or(true)
            {
                backend_models_wrapped(ui, title, models, empty_label);
            }
        }
        if s.enabled_providers.as_ref().map(|v| v.is_empty()).unwrap_or(false) {
            ui.label(
                egui::RichText::new("no backends enabled for provider discovery").weak(),
            );
        }
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);
}

fn status_skill_packages_section(ui: &mut egui::Ui, status: Option<&GatewayStatusDetails>) {
    dashboard::section_group(ui, "Skill packages", |ui| {
        let Some(s) = status else {
            ui.label(egui::RichText::new("Loading from gateway status...").weak());
            return;
        };
        match (
            s.skill_packages_discovery_root.as_deref(),
            s.skill_packages_discovered,
        ) {
            (Some(root), Some(n)) if !root.trim().is_empty() => {
                dashboard::kv(ui, "Discovery root", root.trim());
                dashboard::kv(ui, "Packages discovered", &n.to_string());
            }
            (Some(root), _) if !root.trim().is_empty() => {
                dashboard::kv(ui, "Discovery root", root.trim());
                ui.label(egui::RichText::new("(package count not reported)").weak());
            }
            _ => {
                ui.label(egui::RichText::new("(not reported)").weak());
            }
        }
    });
}

fn status_column_left(
    ui: &mut egui::Ui,
    status: Option<&GatewayStatusDetails>,
    gateway_error: Option<&str>,
) {
    status_clock_section(ui, status);
    status_gateway_section(ui, status, gateway_error);
    status_channels_section(ui, status);
    status_providers_section(ui, status);
}

fn context_mode_for_agent(s: &GatewayStatusDetails, agent_id: &str) -> String {
    let id = agent_id.trim();
    if let Some(m) = s.agent_context_modes.get(id) {
        return m.clone();
    }
    let orch = s
        .orchestrator_id
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or("orchestrator");
    if id == orch {
        if let Some(ref m) = s.context_mode {
            return m.clone();
        }
    }
    "—".to_string()
}

fn status_column_agents(ui: &mut egui::Ui, status: Option<&GatewayStatusDetails>) {
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
            if let Some(ref dir) = s.orchestrator_context_dir {
                let t = dir.trim();
                if !t.is_empty() {
                    dashboard::kv(ui, "context directory", t);
                }
            }
            let dp = s.default_provider.as_deref().unwrap_or("—");
            let dm = s.default_model.as_deref().unwrap_or("—");
            dashboard::kv(ui, "default provider", dp);
            dashboard::kv(ui, "default model", dm);
            dashboard::kv(
                ui,
                "context mode",
                context_mode_for_agent(s, oid).as_str(),
            );

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
                    .num_columns(4)
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
                        dashboard::grid_cell(ui, |ui| {
                            ui.label(dashboard::grid_header_rich(ui, "Context mode"));
                        });
                        ui.end_row();
                        for w in &s.workers {
                            let wid = w.id.trim();
                            dashboard::grid_cell(ui, |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(wid)));
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
                            let cm = context_mode_for_agent(s, wid);
                            dashboard::grid_cell(ui, |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(cm)));
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

    egui::Grid::new(format!("status_models_grid_{title}"))
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
