use eframe::egui;

use super::super::Screen;

fn section_heading(ui: &mut egui::Ui, label: &str, top: f32) {
    ui.add_space(top);
    ui.label(
        egui::RichText::new(label)
            .small()
            .strong()
            .color(ui.style().visuals.weak_text_color()),
    );
    ui.add_space(6.0);
}

/// Render the left sidebar: **Chat** (primary, ungrouped), then Runtime, Source, Diagnostics.
pub fn sidebar(current_screen: &mut Screen, ctx: &egui::Context) {
    egui::SidePanel::left("sidebar")
        .resizable(false)
        .exact_width(152.0)
        .show(ctx, |ui| {
            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                .show(ui, |ui| {
                    ui.add_space(24.0);

                    if ui
                        .selectable_label(*current_screen == Screen::Chat, "Chat")
                        .clicked()
                    {
                        *current_screen = Screen::Chat;
                    }

                    section_heading(ui, "Runtime", 14.0);
                    if ui
                        .selectable_label(*current_screen == Screen::Status, "Status")
                        .clicked()
                    {
                        *current_screen = Screen::Status;
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
                        .selectable_label(*current_screen == Screen::Tools, "Tools")
                        .clicked()
                    {
                        *current_screen = Screen::Tools;
                    }

                    section_heading(ui, "Source", 14.0);
                    if ui
                        .selectable_label(*current_screen == Screen::Config, "Config")
                        .clicked()
                    {
                        *current_screen = Screen::Config;
                    }
                    ui.add_space(12.0);
                    if ui
                        .selectable_label(*current_screen == Screen::Skills, "Skills")
                        .clicked()
                    {
                        *current_screen = Screen::Skills;
                    }

                    section_heading(ui, "Diagnostics", 14.0);
                    if ui
                        .selectable_label(*current_screen == Screen::Logs, "Logs")
                        .clicked()
                    {
                        *current_screen = Screen::Logs;
                    }
                });
        });
}
