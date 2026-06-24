use eframe::egui;

/// Amber text color used for profile-mismatch hints.
const AMBER_TEXT: egui::Color32 = egui::Color32::from_rgb(200, 150, 50);

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
/// `probe_completed` controls whether the Start button is enabled yet.
/// `profile_dropdown_enabled` is false while a gateway holds `gateway.lock` (or when the UI should block switching).
/// `profile_error` is `Some(msg)` when a profile switch failed (shown as right-aligned red text below the header).
/// `gateway_error` is `Some(msg)` when the gateway failed to start or exited unexpectedly (shown as
///   right-aligned red text below the header, visible from any screen).
/// `profile_mismatch` is `Some(label)` when the gateway is running a different profile than the desktop's
///   effective profile (e.g. due to `CHAI_PROFILE` or an externally started gateway); the dropdown
///   is disabled and `label` is shown as an amber hint.
/// `on_profile_change` is called with the selected profile name when the user picks a different profile.
/// `on_start` and `on_stop` are callbacks invoked when the corresponding
/// buttons are pressed.
pub fn header<FProfile, FStart, FStop>(
    ctx: &egui::Context,
    running: bool,
    owned: bool,
    probe_completed: bool,
    profile_names: &[String],
    profile_active: &str,
    profile_dropdown_enabled: bool,
    profile_error: Option<&str>,
    gateway_error: Option<&str>,
    profile_mismatch: Option<&str>,
    mut on_profile_change: FProfile,
    mut on_start: FStart,
    mut on_stop: FStop,
) where
    FProfile: FnMut(String),
    FStart: FnMut(),
    FStop: FnMut(),
{
    // When there is a profile mismatch, the dropdown is disabled.
    let dropdown_enabled =
        profile_dropdown_enabled && profile_mismatch.is_none();

    egui::TopBottomPanel::top("header").show(ctx, |ui| {
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(24.0, 0.0))
            .show(ui, |ui| {
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.heading("Chai");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                // Profile mismatch hint (e.g. CHAI_PROFILE override or gateway running different profile)
                if let Some(label) = profile_mismatch {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (display, truncated) = truncate_label(label, HEADER_LABEL_MAX_CHARS);
                            let response = ui.label(
                                egui::RichText::new(&display)
                                    .color(AMBER_TEXT),
                            );
                            if truncated {
                                response.on_hover_text(label);
                            }
                        });
                    });
                }
                ui.add_space(16.0);
            });
    });
}
