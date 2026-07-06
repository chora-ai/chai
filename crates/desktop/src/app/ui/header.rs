use eframe::egui;

/// Maximum character count for right-aligned header labels before truncation.
/// Long error messages in the header overflow the panel; truncating them with
/// a hover tooltip keeps the header compact while preserving full detail.
const HEADER_LABEL_MAX_CHARS: usize = 80;

/// Truncate a string to `max_chars` with an ellipsis if it exceeds the limit.
/// Returns `(display_text, is_truncated)`.
fn truncate_label(s: &str, max_chars: usize) -> (String, bool) {
    if s.chars().count() <= max_chars {
        (s.to_string(), false)
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        (format!("{}…", truncated), true)
    }
}

/// Render the top header with title, profile switcher, and gateway controls.
///
/// `running` and `owned` describe the current gateway state.
/// `probe_completed` controls whether the Start/Connect button is enabled yet.
/// `is_remote` controls the button labels: Connect/Disconnect for remote profiles,
///   Start/Stop for local profiles.
/// `remote_disconnected` is true when the user explicitly disconnected from a remote
///   profile. In this state, the Connect button is enabled even though `probe_completed`
///   is false, because the probe is suppressed while disconnected.
/// `profile_dropdown_enabled` controls whether the profile ComboBox is interactive (always true
///   with per-profile locks; the switch handler checks per-profile gateway state).
/// `profile_error` is `Some(msg)` when a profile switch failed (shown as right-aligned red text below the header).
/// `gateway_error` is `Some(msg)` when the gateway failed to start or exited unexpectedly (shown as
///   right-aligned red text below the header, visible from any screen).
/// `on_profile_change` is called with the selected profile name when the user picks a different profile.
/// `on_start` and `on_stop` are callbacks invoked when the corresponding
/// buttons are pressed.
pub fn header<FProfile, FStart, FStop>(
    ctx: &egui::Context,
    running: bool,
    owned: bool,
    probe_completed: bool,
    is_remote: bool,
    remote_disconnected: bool,
    profile_names: &[String],
    profile_active: &str,
    profile_dropdown_enabled: bool,
    profile_error: Option<&str>,
    gateway_error: Option<&str>,
    mut on_profile_change: FProfile,
    mut on_start: FStart,
    mut on_stop: FStop,
) where
    FProfile: FnMut(String),
    FStart: FnMut(),
    FStop: FnMut(),
{
    // With per-profile locks, the mismatch hint is informational only.
    // The user can switch profiles freely.
    let dropdown_enabled = profile_dropdown_enabled;

    egui::TopBottomPanel::top("header").show(ctx, |ui| {
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(24.0, 0.0))
            .show(ui, |ui| {
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.heading("Chai");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if is_remote {
                            // Remote profile: Connect/Disconnect buttons.
                            // When the user explicitly disconnected (remote_disconnected),
                            // show the Connect button as enabled even though probe_completed
                            // is false — the probe is suppressed while disconnected, so
                            // probe_completed will never become true until the user clicks
                            // Connect (which clears remote_disconnected and triggers a probe).
                            if !probe_completed && !remote_disconnected {
                                ui.add_enabled(false, egui::Button::new("Connect"));
                            } else if running {
                                if ui.button("Disconnect").clicked() {
                                    on_stop();
                                }
                            } else if ui.button("Connect").clicked() {
                                on_start();
                            }
                        } else {
                            // Local profile: Start/Stop gateway buttons.
                            if !probe_completed {
                                ui.add_enabled(false, egui::Button::new("Start gateway"));
                            } else if running {
                                if owned {
                                    if ui.button("Stop gateway").clicked() {
                                        on_stop();
                                    }
                                } else {
                                    ui.add_enabled(false, egui::Button::new("Gateway running"));
                                }
                            } else if ui.button("Start gateway").clicked() {
                                on_start();
                            }
                        }

                        ui.add_space(8.0);
                        if profile_names.is_empty() {
                            ui.label(egui::RichText::new("no profiles").weak());
                        } else {
                            let selected_text = if profile_active.is_empty() {
                                "profile".to_string()
                            } else {
                                profile_active.to_string()
                            };
                            ui.add_enabled_ui(dropdown_enabled, |ui| {
                                egui::ComboBox::from_id_source("chai_profile_select")
                                    .selected_text(selected_text)
                                    .show_ui(ui, |ui| {
                                        for name in profile_names {
                                            if ui
                                                .selectable_label(profile_active == name, name)
                                                .clicked()
                                                && name != profile_active
                                            {
                                                on_profile_change(name.clone());
                                            }
                                        }
                                    });
                            });
                        }
                    });
                });
                // Error line (e.g. failed profile switch)
                if let Some(err) = profile_error {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (display, truncated) = truncate_label(err, HEADER_LABEL_MAX_CHARS);
                            let label = ui.label(
                                egui::RichText::new(&display)
                                    .color(egui::Color32::from_rgb(200, 64, 64)),
                            );
                            if truncated {
                                label.on_hover_text(err);
                            }
                        });
                    });
                }
                // Gateway error line (e.g. config parse failure, unexpected exit).
                if let Some(err) = gateway_error {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (display, truncated) = truncate_label(err, HEADER_LABEL_MAX_CHARS);
                            let label = ui.label(
                                egui::RichText::new(&display)
                                    .color(egui::Color32::from_rgb(200, 64, 64)),
                            );
                            if truncated {
                                label.on_hover_text(err);
                            }
                        });
                    });
                }
                ui.add_space(16.0);
            });
    });
}
