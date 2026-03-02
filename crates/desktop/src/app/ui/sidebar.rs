use eframe::egui;

use super::super::Screen;

/// Render the left sidebar with screen navigation.
pub fn sidebar(current_screen: &mut Screen, ctx: &egui::Context) {
    egui::SidePanel::left("sidebar")
        .resizable(false)
        .exact_width(140.0)
        .show(ctx, |ui| {
            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                .show(ui, |ui| {
                    ui.add_space(24.0);
                    if ui
                        .selectable_label(*current_screen == Screen::Info, "Info")
                        .clicked()
                    {
                        *current_screen = Screen::Info;
                    }
                    ui.add_space(12.0);
                    if ui
                        .selectable_label(*current_screen == Screen::Chat, "Chat")
                        .clicked()
                    {
                        *current_screen = Screen::Chat;
                    }
                    ui.add_space(12.0);
                    if ui
                        .selectable_label(*current_screen == Screen::Config, "Config")
                        .clicked()
                    {
                        *current_screen = Screen::Config;
                    }
                    ui.add_space(12.0);
                    if ui
                        .selectable_label(*current_screen == Screen::Context, "Context")
                        .clicked()
                    {
                        *current_screen = Screen::Context;
                    }
                    ui.add_space(12.0);
                    if ui
                        .selectable_label(*current_screen == Screen::Skills, "Skills")
                        .clicked()
                    {
                        *current_screen = Screen::Skills;
                    }
                    ui.add_space(12.0);
                    if ui
                        .selectable_label(*current_screen == Screen::Logs, "Logs")
                        .clicked()
                    {
                        *current_screen = Screen::Logs;
                    }
                });
        });
}

