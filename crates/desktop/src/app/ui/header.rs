use eframe::egui;

/// Render the top header with title and gateway controls.
///
/// `running` and `owned` describe the current gateway state.
/// `probe_completed` controls whether the Start button is enabled yet.
/// `on_start` and `on_stop` are callbacks invoked when the corresponding
/// buttons are pressed.
pub fn header<FStart, FStop>(
    ctx: &egui::Context,
    running: bool,
    owned: bool,
    probe_completed: bool,
    mut on_start: FStart,
    mut on_stop: FStop,
) where
    FStart: FnMut(),
    FStop: FnMut(),
{
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
                    });
                });
                ui.add_space(16.0);
            });
    });
}

