use eframe::egui;

use super::super::ChaiApp;

/// Section heading style matching the left sidebar.
fn section_heading(ui: &mut egui::Ui, label: &str) {
    ui.add_space(18.0);
    ui.label(
        egui::RichText::new(label)
            .strong()
            .color(ui.style().visuals.weak_text_color()),
    );
    ui.add_space(12.0);
}

/// Short label for a session in the sessions list (timestamp with optional channel name).
fn session_label_display(summary: &super::super::SessionSummary) -> String {
    // Parse the ISO 8601 timestamp to a short display form like "Jun 10, 12:34".
    let time_label = format_timestamp(&summary.created_at);
    // Only show the channel_id (e.g. "telegram") — the conversation_id is an internal
    // identifier that makes the label overflow the sidebar width.
    if let Some(ref binding) = summary.channel_binding {
        if !binding.channel_id.is_empty() {
            format!("{} ({})", time_label, binding.channel_id)
        } else {
            time_label
        }
    } else {
        time_label
    }
}

/// Format an ISO 8601 timestamp string to a short display form.
/// Returns the raw string if parsing fails.
fn format_timestamp(iso: &str) -> String {
    // Try to parse common ISO 8601 forms: "2025-06-10T12:34:56Z" or "2025-06-10T12:34:56+00:00"
    // Extract date and time components for a short display like "Jun 10, 12:34".
    let s = iso.trim();
    // Expected format: YYYY-MM-DDTHH:MM:SS...
    if s.len() < 16 {
        return s.to_string();
    }
    let month_str = &s[5..7];
    let day_str = &s[8..10];
    let hour_str = &s[11..13];
    let min_str = &s[14..16];

    let month = match month_str {
        "01" => "Jan",
        "02" => "Feb",
        "03" => "Mar",
        "04" => "Apr",
        "05" => "May",
        "06" => "Jun",
        "07" => "Jul",
        "08" => "Aug",
        "09" => "Sep",
        "10" => "Oct",
        "11" => "Nov",
        "12" => "Dec",
        _ => return s.to_string(),
    };

    format!("{} {}, {}:{}", month, day_str, hour_str, min_str)
}

/// Shortened session id for display (first 12 chars after "sess-" prefix).
fn short_session_id(id: &str) -> String {
    if let Some(rest) = id.strip_prefix("sess-") {
        if rest.len() > 12 {
            format!(" sess-{}…", &rest[..12])
        } else {
            id.to_string()
        }
    } else if id.len() > 16 {
        format!(" {}…", &id[..16])
    } else {
        id.to_string()
    }
}

/// Render the right sessions panel when on the chat screen.
pub fn sessions_panel(app: &mut ChaiApp, ctx: &egui::Context, running: bool) {
    if app.current_screen != super::super::Screen::Chat {
        return;
    }

    // Default selected session to current chat session when none selected
    if app.selected_session_id.is_none() && app.chat_session_id.is_some() {
        app.selected_session_id = app.chat_session_id.clone();
    }

    egui::SidePanel::right("sessions_panel")
        .resizable(false)
        .exact_width(220.0)
        .show(ctx, |ui| {
            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                .show(ui, |ui| {
                    ui.add_space(6.0);

                    // ── Agent selector ──
                    section_heading(ui, "Agent");
                    let orchestrator_ids: Vec<String> = app
                        .gateway_status
                        .as_ref()
                        .map(|gs| gs.orchestrators.iter().map(|o| o.id.clone()).collect())
                        .unwrap_or_default();
                    let active_id = app
                        .active_orchestrator_id
                        .as_deref()
                        .or_else(|| orchestrator_ids.first().map(|s| s.as_str()))
                        .unwrap_or("orchestrator");
                    let single_orchestrator = orchestrator_ids.len() <= 1;
                    // Disable when: not running, only one orchestrator, or a chat turn is in progress.
                    let combo_enabled = running
                        && !single_orchestrator
                        && app.chat_turn_receiver.is_none()
                        && !app.chat_stopping;

                    let mut selected_id = active_id.to_string();
                    ui.add_enabled_ui(combo_enabled, |ui| {
                        let width = ui.available_width();
                        egui::ComboBox::from_id_source("active_orchestrator_select")
                            .width(width)
                            .selected_text(active_id)
                            .show_ui(ui, |ui| {
                                for id in &orchestrator_ids {
                                    ui.selectable_value(&mut selected_id, id.clone(), id);
                                }
                            });
                    });
                    if selected_id != active_id {
                        app.switch_active_orchestrator(selected_id);
                    }

                    // ── Sessions ──
                    section_heading(ui, "Sessions");

                    if !running {
                        ui.label("Start the gateway to see sessions.");
                    } else {
                        if ui.button("New session").clicked() {
                            app.start_new_session();
                        }
                        ui.add_space(12.0);

                        // Session list from session_order (all sessions, not just those with loaded messages).
                        for session_id in app.session_order.iter().cloned().collect::<Vec<_>>() {
                            let is_selected =
                                app.selected_session_id.as_deref() == Some(session_id.as_str());
                            let is_deleting = app
                                .sessions_delete_receiver
                                .as_ref()
                                .map_or(false, |(id, _)| id == &session_id);

                            let summary = app.session_summaries.get(&session_id);
                            let display = summary
                                .map(|s| session_label_display(s))
                                .unwrap_or_else(|| short_session_id(&session_id));

                            // Same pattern as header.rs: selectable label on the left,
                            // delete button in a right-to-left section on the right.
                            // The RTL section reserves space from the right edge, so the
                            // label is naturally constrained and won't push the button off screen.
                            ui.horizontal(|ui| {
                                let response = ui.selectable_label(is_selected, display);
                                if response.clicked() {
                                    // If the session's messages aren't loaded yet, trigger a history fetch.
                                    if !app.session_messages.contains_key(&session_id)
                                        && app.loading_session_id.is_none()
                                        && app.sessions_history_receiver.is_none()
                                    {
                                        app.loading_session_id = Some(session_id.clone());
                                        let profile_override = app.cached_profile_override.clone();
                                        let sid = session_id.clone();
                                        let (tx, rx) = std::sync::mpsc::channel();
                                        std::thread::spawn(move || {
                                            let result = super::super::state::gateway::fetch_sessions_history(
                                                profile_override.as_deref(),
                                                &sid,
                                            );
                                            let _ = tx.send(result);
                                        });
                                        app.sessions_history_receiver = Some((session_id.clone(), rx));
                                    }
                                    app.selected_session_id = Some(session_id.clone());
                                    // Only set chat_session_id for non-channel sessions.
                                    // Channel-bound sessions can only be updated through their
                                    // channel (e.g. Telegram); sending from the desktop would
                                    // create a new empty session on the gateway, overwriting the
                                    // channel session's history.
                                    let is_channel_session = summary
                                        .as_ref()
                                        .and_then(|s| s.channel_binding.as_ref())
                                        .is_some_and(|b| !b.channel_id.is_empty());
                                    if !is_channel_session {
                                        app.chat_session_id = Some(session_id.clone());
                                    }
                                }

                                // Delete button (right-aligned via RTL layout).
                                // Disable for the active session while the agent is running.
                                let is_active = app.chat_session_id.as_deref() == Some(session_id.as_str());
                                let turn_in_progress = app.chat_turn_receiver.is_some() || app.chat_stopping;
                                let can_delete = !(is_active && turn_in_progress);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if !is_deleting {
                                        let delete_button = ui.add_enabled(can_delete, egui::Button::new("×").small());
                                        if delete_button.clicked() {
                                            let profile_override = app.cached_profile_override.clone();
                                            let sid = session_id.clone();
                                            let (tx, rx) = std::sync::mpsc::channel();
                                            std::thread::spawn(move || {
                                                let result = super::super::state::gateway::fetch_sessions_delete(
                                                    profile_override.as_deref(),
                                                    &sid,
                                                );
                                                let _ = tx.send(result);
                                            });
                                            app.sessions_delete_receiver = Some((session_id.clone(), rx));
                                        }
                                    } else {
                                        ui.label(egui::RichText::new("…").weak());
                                    }
                                });
                            });

                            // Show short session id as a secondary label.
                            if summary.is_some() {
                                ui.label(
                                    egui::RichText::new(short_session_id(&session_id))
                                        .weak(),
                                );
                            }

                            ui.add_space(4.0);
                        }

                        if app.session_order.is_empty() && app.sessions_list_fetched {
                            ui.label("No sessions yet. Send a message to start one.");
                        }

                        // "Clear all" button at the bottom.
                        // Disable while the agent is running (a turn is in progress).
                        let turn_in_progress = app.chat_turn_receiver.is_some() || app.chat_stopping;
                        if !app.session_order.is_empty() {
                            ui.add_space(8.0);
                            if ui.add_enabled(!turn_in_progress, egui::Button::new("Clear all sessions")).clicked() {
                                app.show_clear_all_confirm = true;
                            }
                        }

                        // Confirmation dialog for "Clear all".
                        if app.show_clear_all_confirm {
                            ui.add_space(4.0);
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 165, 0),
                                "Delete all sessions?",
                            );
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if ui.button("Yes, clear all").clicked() {
                                    app.show_clear_all_confirm = false;
                                    let profile_override = app.cached_profile_override.clone();
                                    let orchestrator_id = app.active_orchestrator_id.clone();
                                    let (tx, rx) = std::sync::mpsc::channel();
                                    std::thread::spawn(move || {
                                        let result = super::super::state::gateway::fetch_sessions_delete_all(
                                            profile_override.as_deref(),
                                            orchestrator_id.as_deref(),
                                        );
                                        let _ = tx.send(result);
                                    });
                                    app.sessions_delete_all_receiver = Some(rx);
                                }
                                if ui.button("Cancel").clicked() {
                                    app.show_clear_all_confirm = false;
                                }
                            });
                        }
                    }
                    ui.add_space(ChaiApp::SCREEN_FOOTER_SPACING);
                });
        });
}