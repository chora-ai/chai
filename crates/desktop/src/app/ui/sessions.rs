use eframe::egui;

use super::super::ChaiApp;

/// Short label for a session in the sessions list (id with optional channel/conversation).
fn session_label_display(
    session_id: &str,
    meta: Option<&(Option<String>, Option<String>)>,
) -> String {
    match meta {
        Some((Some(cid), Some(conv))) => format!("{} ({}:{})", session_id, cid, conv),
        Some((Some(cid), None)) => format!("{} ({})", session_id, cid),
        _ => session_id.to_string(),
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
                    ui.add_space(24.0);
                    ui.heading("Sessions");
                    ui.add_space(ChaiApp::SCREEN_TITLE_BOTTOM_SPACING);
                    if !running {
                        ui.label("Start the gateway to see sessions.");
                    } else {
                        if app.chat_session_id.is_none() {
                            if ui.button("New session").clicked() {
                                app.selected_session_id = None;
                            }
                            ui.add_space(8.0);
                        }
                        for session_id in app
                            .session_order
                            .iter()
                            .filter(|id| app.session_messages.contains_key(*id))
                            .cloned()
                            .collect::<Vec<_>>()
                        {
                            let is_selected =
                                app.selected_session_id.as_deref() == Some(session_id.as_str());
                            let display =
                                session_label_display(&session_id, app.session_meta.get(&session_id));
                            if ui.selectable_label(is_selected, display).clicked() {
                                app.selected_session_id = Some(session_id);
                            }
                        }
                        if app.session_messages.is_empty() {
                            ui.label("No sessions yet. Send a message to start one.");
                        }
                    }
                    ui.add_space(ChaiApp::SCREEN_FOOTER_SPACING);
                });
        });
}

