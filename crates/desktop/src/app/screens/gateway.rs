use eframe::egui;
use serde_json::Value as JsonValue;

use crate::app::ui::{dashboard, readonly_code, spacing, view_toggle};
use crate::app::{ChaiApp, GatewayStatusDetails, StatusViewMode};

pub fn ui_gateway_screen(app: &mut ChaiApp, ui: &mut egui::Ui, running: bool) {
    crate::app::ui_screen(
        ui,
        "Gateway",
        Some(if running {
            "Values loaded from connected gateway."
        } else {
            "Start the gateway to load gateway status."
        }),
        |ui| {
            // Show gateway error outside the running guard so the user can see
            // it even when the gateway failed to start (e.g. config parse error,
            // missing binary, or unexpected process exit).
            if let Some(ref err) = app.gateway_error {
                ui.label(egui::RichText::new(err).color(egui::Color32::RED));
                ui.add_space(spacing::SUBSECTION);
            }

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
                                // Raw JSON was not included in the last status fetch
                                // (computed on-demand only). Request an immediate
                                // refetch so the next poll includes it.
                                app.request_status_refetch();
                                ui.label(
                                    egui::RichText::new("Loading raw JSON…")
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
                    );
                });
        },
    );
}

fn status_summary_dashboard(
    ui: &mut egui::Ui,
    status: Option<&GatewayStatusDetails>,
) {
    dashboard::dashboard_two_columns(ui, |left, right| {
        left.vertical(|ui| {
            status_column_left(ui, status);
        });
        right.vertical(|ui| {
            status_sandbox_section(ui, status);
            ui.add_space(spacing::DASHBOARD_COLUMN_GAP);
            status_agents_section(ui, status);
            ui.add_space(spacing::DASHBOARD_COLUMN_GAP);
            status_skills_section(ui, status);
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

fn status_gateway_section(
    ui: &mut egui::Ui,
    status: Option<&GatewayStatusDetails>,
) {
    dashboard::section_group(ui, "Gateway", |ui| {
        if let Some(s) = status {
            dashboard::kv(ui, "Bind", s.bind.trim());
            dashboard::kv(ui, "Port", &s.port.to_string());
            dashboard::kv(ui, "Auth", s.auth.trim());
            dashboard::kv(ui, "Protocol", &s.protocol.to_string());
            let st = s.status.trim();
            dashboard::kv(
                ui,
                "Status",
                if st.is_empty() { "(not reported)" } else { st },
            );
        } else {
            ui.label(egui::RichText::new("Loading from gateway status...").weak());
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
            ui.label(egui::RichText::new("(no channels)").weak());
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
                dashboard::kv(ui, "Value", &channel_status_field_display(per, 120));
                continue;
            };
            let mut keys: Vec<_> = po.keys().cloned().collect();
            keys.sort();
            for k in keys {
                let Some(val) = po.get(&k) else {
                    continue;
                };
                // Capitalize first letter of the camelCase key for display.
                let display_key = format_display_key(&k);
                dashboard::kv(ui, &display_key, &channel_status_field_display(val, 160));
            }
        }
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);
}

/// Convert a camelCase or snake_case JSON key to the preferred display format:
/// uppercase first letter of first word, single space between words.
fn format_display_key(key: &str) -> String {
    // Split camelCase: insert space before uppercase letters that follow lowercase.
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    for ch in key.chars() {
        if ch.is_uppercase() && !current.is_empty() {
            words.push(current.clone());
            current.clear();
            current.extend(ch.to_lowercase());
        } else if ch == '_' {
            if !current.is_empty() {
                words.push(current.clone());
                current.clear();
            }
        } else {
            current.extend(ch.to_lowercase());
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    if words.is_empty() {
        return key.to_string();
    }
    // Uppercase first letter of first word; rest stay lowercase.
    let mut result = String::new();
    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            result.push(' ');
        }
        if i == 0 {
            if let Some(first) = word.chars().next() {
                result.extend(first.to_uppercase());
                result.push_str(&word[first.len_utf8()..]);
            }
        } else {
            result.push_str(word);
        }
    }
    result
}

fn status_providers_section(ui: &mut egui::Ui, status: Option<&GatewayStatusDetails>) {
    dashboard::section_group(ui, "Providers", |ui| {
        let Some(s) = status else {
            ui.label(egui::RichText::new("Loading from gateway status...").weak());
            return;
        };

        // Iterate providers dynamically from the parsed provider_info.
        let mut provider_ids: Vec<&String> = s.provider_info.keys().collect();
        provider_ids.sort();

        for pid in &provider_ids {
            let info = s.provider_info.get(pid.as_str()).unwrap();
            ui.add_space(spacing::SUBSECTION_HEADING_GAP);
            ui.label(egui::RichText::new(pid.as_str()).strong());
            ui.add_space(spacing::SUBSECTION_HEADING_GAP);
            dashboard::kv(ui, "Endpoint type", &info.endpoint_type);
            dashboard::kv(ui, "Model discovery", &info.model_discovery);

            if info.models.is_empty() {
                if info.model_discovery.contains("static") {
                    dashboard::kv(ui, "Models", "(no models provided)");
                } else {
                    dashboard::kv(ui, "Models", "(no models discovered)");
                }
            } else {
                let label = format!("Models — {} model{}", info.models.len(), if info.models.len() == 1 { "" } else { "s" });
                ui.add_space(spacing::COLLAPSING_HEADER_BEFORE);
                egui::CollapsingHeader::new(&label)
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.add_space(spacing::COLLAPSING_BODY_INSET);
                        egui::Grid::new(format!("status_provider_models_grid_{pid}"))
                            .num_columns(1)
                            .striped(true)
                            .spacing([spacing::GRID_CELL_SPACING, spacing::GRID_CELL_SPACING])
                            .show(ui, |ui| {
                                for m in &info.models {
                                    dashboard::grid_cell(ui, |ui| {
                                        ui.add(egui::Label::new(egui::RichText::new(m)));
                                    });
                                    ui.end_row();
                                }
                            });
                        ui.add_space(spacing::COLLAPSING_BODY_INSET);
                    });
            }
            ui.add_space(spacing::TABLE_BLOCK_AFTER);
        }

        if provider_ids.is_empty() {
            ui.label(egui::RichText::new("(no providers configured)").weak());
        }
    });
    ui.add_space(spacing::DASHBOARD_COLUMN_GAP);
}

fn status_column_left(
    ui: &mut egui::Ui,
    status: Option<&GatewayStatusDetails>,
) {
    status_gateway_section(ui, status);
    status_channels_section(ui, status);
    status_providers_section(ui, status);
}

fn context_mode_for_agent(s: &GatewayStatusDetails, agent_id: &str) -> String {
    let id = agent_id.trim();
    if let Some(m) = s.agent_context_modes.get(id) {
        return m.clone();
    }
    if let Some(rt) = s.agent_skills.get(id) {
        if let Some(ref m) = rt.context_mode {
            return m.clone();
        }
    }
    "—".to_string()
}

fn status_sandbox_section(ui: &mut egui::Ui, status: Option<&GatewayStatusDetails>) {
    dashboard::section_group(ui, "Sandbox", |ui| {
        if let Some(s) = status {
            dashboard::kv(ui, "Mode", s.sandbox_mode.as_str());
            dashboard::kv(ui, "Roots", &s.sandbox_roots.to_string());
        } else {
            ui.label(egui::RichText::new("Loading from gateway status...").weak());
        }
    });
}

fn status_agents_section(ui: &mut egui::Ui, status: Option<&GatewayStatusDetails>) {
    dashboard::section_group(ui, "Agents", |ui| {
        if let Some(s) = status {
            // Orchestrator subsections — each title is just the lowercase agent id.
            for orch in &s.orchestrators {
                let oid = orch.id.trim();
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                ui.label(egui::RichText::new(oid).strong());
                ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                dashboard::kv(ui, "Role", "orchestrator");
                let dp = orch.default_provider.trim();
                let dm = orch.default_model.trim();
                dashboard::kv(ui, "Default provider", if dp.is_empty() { "—" } else { dp });
                dashboard::kv(ui, "Default model", if dm.is_empty() { "—" } else { dm });
                let orch_ep = {
                    let v: Vec<String> = orch
                        .enabled_providers
                        .iter()
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect();
                    if v.is_empty() {
                        "(none)".to_string()
                    } else {
                        v.join(", ")
                    }
                };
                dashboard::kv(ui, "Enabled providers", orch_ep.as_str());
                let orch_skills_csv = orch.enabled_skills.join(", ");
                dashboard::kv(
                    ui,
                    "Enabled skills",
                    if orch.enabled_skills.is_empty() {
                        "(none)"
                    } else {
                        orch_skills_csv.as_str()
                    },
                );
                dashboard::kv(ui, "Context mode", context_mode_for_agent(s, oid).as_str());

                // Enabled workers.
                match &orch.enabled_workers {
                    None => dashboard::kv(ui, "Enabled workers", "(none)"),
                    Some(ew) if ew.is_empty() => dashboard::kv(ui, "Enabled workers", "(all)"),
                    Some(ew) => dashboard::kv(ui, "Enabled workers", ew.join(", ").as_str()),
                }

                // Orchestrator limit fields (same order as config screen).
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

            // Worker subsections — each title is just the lowercase worker id.
            if !s.workers.is_empty() {
                for w in &s.workers {
                    let wid = w.id.trim();
                    ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                    ui.label(egui::RichText::new(wid).strong());
                    ui.add_space(spacing::SUBSECTION_HEADING_GAP);
                    dashboard::kv(ui, "Role", "worker");
                    let wp = w.default_provider.trim();
                    let wm = w.default_model.trim();
                    dashboard::kv(ui, "Default provider", if wp.is_empty() { "—" } else { wp });
                    dashboard::kv(ui, "Default model", if wm.is_empty() { "—" } else { wm });
                    let skills_list = w.enabled_skills.join(", ");
                    dashboard::kv(
                        ui,
                        "Enabled skills",
                        if w.enabled_skills.is_empty() {
                            "(none)"
                        } else {
                            skills_list.as_str()
                        },
                    );
                    let cm = w.context_mode.as_deref().unwrap_or("—");
                    dashboard::kv(ui, "Context mode", cm);
                }
            }
        } else {
            ui.label(egui::RichText::new("Loading from gateway status...").weak());
        }
    });
}

fn status_skills_section(ui: &mut egui::Ui, status: Option<&GatewayStatusDetails>) {
    dashboard::section_group(ui, "Skills", |ui| {
        let Some(s) = status else {
            ui.label(egui::RichText::new("Loading from gateway status...").weak());
            return;
        };
        if let Some(mode) = s.skills_lock_mode.as_deref() {
            dashboard::kv(ui, "Lock mode", mode);
        }
        if let Some(gen) = s.skills_lock_generation {
            dashboard::kv(ui, "Lock generation", &gen.to_string());
        } else {
            dashboard::kv(ui, "Lock generation", "(no lockfile)");
        }
        if let Some(count) = s.skills_locked_count {
            dashboard::kv(ui, "Locked count", &count.to_string());
        }
        if let Some(n) = s.skills_packages_discovered {
            dashboard::kv(ui, "Packages Discovered", &n.to_string());
        } else {
            ui.label(egui::RichText::new("(none)").weak());
        }
    });
}
