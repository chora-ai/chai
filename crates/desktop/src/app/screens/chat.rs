use eframe::egui;

use crate::app::{ChaiApp, ChatMessage};
use lib::orchestration::{
    EVENT_DELEGATE_COMPLETE, EVENT_DELEGATE_ERROR, EVENT_DELEGATE_REJECTED, EVENT_DELEGATE_START,
};

const CHAT_INPUT_HEIGHT: f32 = 148.0; // 8 rows
const CHAT_MESSAGES_MIN_HEIGHT: f32 = 80.0;

/// Amber border color used for tool loop limit warnings.
const AMBER_BORDER: egui::Color32 = egui::Color32::from_rgb(90, 70, 30);
/// Blue border color used for delegation events, worker tool calls, and worker replies.
const BLUE_BORDER: egui::Color32 = egui::Color32::from_rgb(70, 70, 90);
/// Green border color used for orchestrator tool calls and assistant messages.
const GREEN_BORDER: egui::Color32 = egui::Color32::from_rgb(70, 90, 70);

pub fn ui_chat_screen(app: &mut ChaiApp, ui: &mut egui::Ui, running: bool) {
    let subtitle = if !running {
        Some("Start the gateway to chat with the orchestrator.")
    } else {
        Some("Chat with the orchestrator using the selected provider and model.")
    };
    crate::app::ui_screen(ui, "Chat", subtitle, |ui| {
        let can_send_base = running
            && (app.selected_session_id == app.chat_session_id
                || (app.selected_session_id.is_none() && app.session_messages.is_empty()));
        let mut can_send = can_send_base;

        let row_height = ui.spacing().interact_size.y + 8.0;
        let bottom_section_height = CHAT_INPUT_HEIGHT + 8.0 + row_height;
        let available = ui.available_height();
        let messages_height = (available - bottom_section_height).max(CHAT_MESSAGES_MIN_HEIGHT);

        let messages_width = ui.available_width();
        let messages_rect = ui
            .allocate_exact_size(
                egui::vec2(messages_width, messages_height),
                egui::Sense::hover(),
            )
            .0;
        let mut messages_ui = ui.child_ui(messages_rect, egui::Layout::top_down(egui::Align::Min));
        // Always use session_messages for the selected session when present to avoid duplicates from chat_messages diverging.
        let messages_to_show: Vec<ChatMessage> = if let Some(ref id) = app.selected_session_id {
            app.session_messages.get(id).cloned().unwrap_or_default()
        } else {
            app.chat_messages.clone()
        };
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(&mut messages_ui, |ui| {
                // Force scroll content to be at least viewport width so the scrollbar stays on the right
                let content_width = ui.available_width();
                ui.allocate_exact_size(egui::vec2(content_width, 0.0), egui::Sense::hover());
                let user_display = local_user_display_name();
                let orchestrator_id = app
                    .gateway_status
                    .as_ref()
                    .and_then(|s| s.orchestrator_id.as_deref())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("orchestrator");
                for (idx, m) in messages_to_show.iter().enumerate() {
                    render_chat_message(ui, idx, m, &user_display, orchestrator_id);
                    ui.add_space(8.0);
                }
            });

        ui.add_space(8.0);

        let text_response = ui.add_enabled_ui(can_send_base, |ui| {
            egui::ScrollArea::vertical()
                .max_height(CHAT_INPUT_HEIGHT)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut app.chat_input)
                            .desired_rows(10)
                            .desired_width(ui.available_width()),
                    )
                })
                .inner
        });
        let response = text_response.inner;
        ui.add_space(8.0);

        let row_width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(row_width, row_height), egui::Sense::hover());
        let mut row_ui = ui.child_ui(rect, egui::Layout::right_to_left(egui::Align::Center));
        egui::Frame::none()
            .inner_margin(egui::Margin {
                left: 0.0,
                right: 8.0,
                top: 4.0,
                bottom: 4.0,
            })
            .show(&mut row_ui, |ui| {
                // Right-to-left layout: first added = rightmost. Left-to-right: Provider, Model, /help, /new, Send.
                // Compute enabled providers list first so it can inform the effective provider fallback.
                let enabled_providers_list = app.enabled_providers();
                let effective_provider = app
                    .current_provider
                    .as_deref()
                    .or_else(|| {
                        app.gateway_status
                            .as_ref()
                            .and_then(|s| s.default_provider.as_deref())
                    })
                    .or_else(|| {
                        app.gateway_status
                            .as_ref()
                            .and_then(|s| s.provider_info.keys().next().map(|k| k.as_str()))
                    })
                    .or_else(|| enabled_providers_list.first().map(|s| s.as_str()))
                    .unwrap_or("ollama")
                    .to_string();
                // Only models for the selected provider (from dynamic provider_info).
                let gateway_models: Vec<String> = app
                    .gateway_status
                    .as_ref()
                    .and_then(|s| s.provider_info.get(&effective_provider).map(|p| p.models.clone()))
                    .unwrap_or_default();
                let effective_default_model = app
                    .gateway_status
                    .as_ref()
                    .and_then(|s| s.default_model.clone())
                    .or_else(|| app.default_model.clone());

                // Determine if this provider is a hosted API (remote endpoint that may not support
                // model discovery but still has models available). Local providers (ollama,
                // openai-compat with local base URL) require a model list before sending; remote
                // providers can be sent to with a default.
                let endpoint_type = app
                    .gateway_status
                    .as_ref()
                    .and_then(|s| s.provider_info.get(&effective_provider).map(|p| p.endpoint_type.as_str()));
                // OpenAI-compat providers with a remote base URL are also treated as hosted.
                // Since we don't have the base URL here, check if the endpoint type is openai-compat
                // and the model list is empty — this covers remote OpenAI-compat providers.
                // For local openai-compat (vLLM, LM Studio etc.), discovery typically succeeds.
                let treat_as_hosted = endpoint_type == Some("openai-compat") && gateway_models.is_empty();

                // Model dropdown: only models for the selected provider. For hosted API providers, use default when list empty.
                let model_options: Vec<String> = if gateway_models.is_empty() && treat_as_hosted {
                    effective_default_model
                        .clone()
                        .map(|m| vec![m])
                        .unwrap_or_else(|| vec!["default".to_string()])
                } else {
                    gateway_models
                };
                // For hosted API providers, allow send even when the gateway has not yet returned a model list.
                let model_available = !model_options.is_empty() || treat_as_hosted;
                can_send = can_send && model_available && !app.chat_turn_receiver.is_some();

                let mut send_now = false;
                let turn_in_progress = app.chat_turn_receiver.is_some();

                let send_button = ui
                    .add_enabled(can_send, egui::Button::new("Send"))
                    .on_hover_text("ctrl/cmd+enter to send");

                // Stop button: shows "Stopping…" while a stop request is in progress.
                ui.add_space(4.0);
                let stopping = app.chat_stopping;
                let can_stop = turn_in_progress && !stopping;
                let stop_label = if stopping { "Stopping…" } else { "Stop" };
                let stop_hover = if stopping {
                    "waiting for current iteration to finish"
                } else {
                    "stop the current turn after this iteration"
                };
                if ui
                    .add_enabled(can_stop, egui::Button::new(stop_label))
                    .on_hover_text(stop_hover)
                    .clicked()
                {
                    app.stop_chat_turn();
                }
                if !model_options.is_empty() {
                    ui.add_space(8.0);
                    let current_label = app
                        .current_model
                        .as_deref()
                        .or(effective_default_model.as_deref())
                        .unwrap_or("—")
                        .to_string();
                    ui.add_enabled_ui(can_send, |ui| {
                        egui::ComboBox::from_id_source("model_select")
                            .selected_text(current_label.as_str())
                            .show_ui(ui, |ui| {
                                for m in &model_options {
                                    let selected = app
                                        .current_model
                                        .as_deref()
                                        .map(|cm| cm == m.as_str())
                                        .unwrap_or(false);
                                    if ui.selectable_label(selected, m).clicked() {
                                        app.current_model = Some(m.clone());
                                    }
                                }
                            });
                    });
                }

                // Provider dropdown: only show enabled providers (from config, cached).
                ui.add_space(8.0);
                if !enabled_providers_list.is_empty() {
                    let selected = if enabled_providers_list.contains(&effective_provider) {
                        effective_provider.clone()
                    } else {
                        enabled_providers_list
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "—".to_string())
                    };
                    ui.add_enabled_ui(can_send, |ui| {
                        egui::ComboBox::from_id_source("provider_select")
                            .selected_text(selected)
                            .show_ui(ui, |ui| {
                                for b in &enabled_providers_list {
                                    if ui
                                        .selectable_label(effective_provider == b.as_str(), b)
                                        .clicked()
                                    {
                                        app.current_provider = Some(b.clone());
                                        app.current_model = None;
                                        app.request_status_refetch();
                                    }
                                }
                            });
                    });
                }

                ui.add_space(8.0);
                if ui
                    .add_enabled(can_send_base, egui::Button::new("/help"))
                    .clicked()
                {
                    app.show_chat_help();
                }
                ui.add_space(8.0);
                if ui
                    .add_enabled(can_send_base, egui::Button::new("/new"))
                    .clicked()
                {
                    app.start_new_session();
                }

                if send_button.clicked() {
                    send_now = true;
                }
                if can_send && response.has_focus() {
                    let modifiers = ui.input(|i| i.modifiers);
                    if modifiers.command || modifiers.ctrl {
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            send_now = true;
                        }
                    }
                }
                if send_now {
                    app.start_chat_turn();
                }
            });

    });
}

/// Local account name for chat labels (`USER` / `USERNAME`), else a generic label.
fn local_user_display_name() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "You".to_string())
}

/// Pretty-print a tool message's `content` when it is valid JSON (e.g. `delegate_task`); otherwise show unchanged.
fn format_tool_content_display(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| content.to_string())
}

/// Returns true when this message is a worker-sourced tool call or tool result
/// (should use the blue border matching delegation events). Any non-orchestrator
/// source is treated as a worker (the source field carries the worker id).
fn is_worker_tool(m: &ChatMessage) -> bool {
    m.source
        .as_deref()
        .is_some_and(|s| s != "orchestrator")
}

/// Renders a single chat message in the same style as the chat screen (frame, role-based fill, content, tool calls).
fn render_chat_message(
    ui: &mut egui::Ui,
    index: usize,
    m: &ChatMessage,
    user_display: &str,
    orchestrator_id: &str,
) {
    let is_user = m.role == "user";
    let is_assistant = m.role == "assistant";
    let is_assistant_progress = m.role == "assistant_progress";
    let is_worker_reply = m.role == "worker";
    let is_error = m.role == "error";
    let is_delegation = m.delegation_event.is_some();
    let is_tool_call = m.role == "tool_call";
    let is_tool_result = m.role == "tool_result";
    let is_tool_loop_limit = m.role == "tool_loop_limit";
    let is_turn_stopped = m.role == "turn_stopped";
    let is_worker = is_worker_tool(m);

    // Skip assistant and assistant_progress messages with empty content — they
    // render as an empty frame with just the orchestrator label, which adds no
    // useful information (e.g. when the tool loop limit is reached and the model
    // only produced tool calls without text).
    if (is_assistant || is_assistant_progress) && m.content.trim().is_empty() {
        return;
    }

    let frame = egui::Frame::none()
        .fill(if is_user {
            ui.style().visuals.extreme_bg_color
        } else if is_tool_loop_limit || is_turn_stopped {
            ui.style().visuals.faint_bg_color
        } else if is_delegation || is_worker || is_worker_reply {
            ui.style().visuals.faint_bg_color
        } else if is_tool_call || is_tool_result {
            ui.style().visuals.faint_bg_color
        } else if is_assistant || is_assistant_progress {
            ui.style().visuals.faint_bg_color
        } else {
            ui.style().visuals.panel_fill
        })
        .stroke(egui::Stroke::new(
            1.0,
            if is_error {
                egui::Color32::RED
            } else if is_tool_loop_limit || is_turn_stopped {
                AMBER_BORDER
            } else if is_delegation || is_worker || is_worker_reply {
                BLUE_BORDER
            } else if is_tool_call || is_tool_result {
                GREEN_BORDER
            } else if is_assistant || is_assistant_progress {
                GREEN_BORDER
            } else {
                ui.style().visuals.widgets.noninteractive.bg_stroke.color
            },
        ))
        .rounding(egui::Rounding::same(12.0))
        .inner_margin(egui::Margin::same(12.0));

    frame.show(ui, |ui| {
        if is_delegation {
            let accent = match m.delegation_event.as_deref() {
                Some(s) if s == EVENT_DELEGATE_START => egui::Color32::from_rgb(70, 110, 180),
                Some(s) if s == EVENT_DELEGATE_COMPLETE => egui::Color32::from_rgb(60, 140, 90),
                Some(s) if s == EVENT_DELEGATE_REJECTED => egui::Color32::from_rgb(180, 120, 40),
                Some(s) if s == EVENT_DELEGATE_ERROR => egui::Color32::from_rgb(180, 60, 60),
                _ => ui.style().visuals.weak_text_color(),
            };
            ui.label(
                egui::RichText::new(&m.content)
                    .italics()
                    .color(accent),
            );
        } else if is_worker_reply {
            // Worker reply — rendered as a first-class chat line showing the
            // worker's response text directly, not buried inside a tool result.
            let worker_id = m.source.as_deref().unwrap_or("worker");
            ui.label(egui::RichText::new(worker_id).small().weak());
            ui.add_space(8.0);
            ui.label(&m.content);
        } else if is_tool_call {
            // Streamed tool call — rendered as a separate timeline entry.
            let tool_name = m.tool_name.as_deref().unwrap_or("unknown");
            let has_result = m.tool_result.is_some();
            let status_icon = if has_result { "✅" } else { "⚙" };
            let label = format!("🔧 {} {}", tool_name, status_icon);
            egui::CollapsingHeader::new(label)
                .id_source(format!("tool_call_{}_{}", index, m.tool_index.unwrap_or(index)))
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(ref args) = m.tool_args {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("Arguments:").small().weak(),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(
                                serde_json::to_string_pretty(args)
                                    .unwrap_or_else(|_| args.to_string()),
                            )
                            .small()
                            .monospace(),
                        );
                    }
                    if let Some(ref result) = m.tool_result {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("Result:").small().weak(),
                        );
                        ui.add_space(8.0);
                        let shown = format_tool_content_display(result);
                        ui.label(
                            egui::RichText::new(shown)
                            .small()
                            .monospace(),
                        );
                    }
                });
        } else if is_tool_result {
            // Standalone tool result (shouldn't normally appear — results are
            // merged into the tool_call entry — but handle gracefully).
            let tool_name = m.tool_name.as_deref().unwrap_or("unknown");
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(format!("🔧 {} ✅", tool_name))
                    .small(),
            );
            if let Some(ref result) = m.tool_result {
                ui.add_space(8.0);
                let shown = format_tool_content_display(result);
                ui.label(egui::RichText::new(shown).small().monospace());
            }
        } else if is_assistant_progress {
            // Intermediate message from the model during tool loop iterations.
            // This is content the model produced alongside tool calls that would
            // otherwise be invisible to the user.
            let label = if is_worker {
                m.source.as_deref().unwrap_or("worker")
            } else {
                orchestrator_id
            };
            ui.label(egui::RichText::new(label).small().weak());
            ui.add_space(8.0);
            ui.label(&m.content);
        } else if is_user {
            ui.label(egui::RichText::new(user_display).small().weak());
            ui.add_space(8.0);
            ui.label(egui::RichText::new(&m.content).strong());
        } else if is_error {
            ui.label(egui::RichText::new("error").small().weak());
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(&m.content)
                    .strong()
                    .color(egui::Color32::RED),
            );
        } else if is_tool_loop_limit {
            // Tool loop iteration limit banner — amber-bordered warning.
            let amber = egui::Color32::from_rgb(200, 150, 50);
            ui.label(
                egui::RichText::new("⚠ tool loop iteration limit reached")
                    .strong()
                    .color(amber),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "The agent's last tool call was not executed. Send another message \
                     to continue the session and ask the agent to retry.",
                )
                .small(),
            );
            if let Some(ref pending) = m.pending_tool_calls {
                if !pending.is_empty() {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("Tool calls not executed:").small().weak(),
                    );
                    ui.add_space(4.0);
                    for call in pending {
                        let name = call
                            .get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown");
                        ui.label(
                            egui::RichText::new(format!("  ⏸ {}", name))
                                .small()
                                .monospace()
                                .color(amber),
                        );
                    }
                }
            }
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(
                    "Adjust maxToolLoopsPerTurn in config.json to set a limit (omitted = no limit).",
                )
                .small()
                .weak(),
            );
        } else if is_turn_stopped {
            // Turn stopped banner — amber-bordered info message.
            let amber = egui::Color32::from_rgb(200, 150, 50);
            ui.label(
                egui::RichText::new("⏸ turn paused")
                    .strong()
                    .color(amber),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "The agent turn was stopped after the current iteration. \
                     The session transcript is preserved — send a new message to continue.",
                )
                .small(),
            );
        } else if is_assistant {
            // Assistant message — show text content only.
            ui.label(egui::RichText::new(orchestrator_id).small().weak());
            ui.add_space(8.0);
            ui.label(&m.content);
        } else {
            ui.label(egui::RichText::new(&m.content));
        }
    });
}
