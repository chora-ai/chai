use eframe::egui;

use super::super::Screen;

fn section_heading(ui: &mut egui::Ui, label: &str) {
    ui.add_space(18.0);
    ui.label(
        egui::RichText::new(label)
            .strong()
            .color(ui.style().visuals.weak_text_color()),
    );
    ui.add_space(12.0);
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
                    ui.add_space(12.0);
                    // TODO: see base/epic/DESKTOP_FILES.md
                    // if ui
                    //     .selectable_label(*current_screen == Screen::Files, "Files")
                    //     .clicked()
                    // {
                    //     *current_screen = Screen::Files;
                    // }
                    // ui.add_space(12.0);
                    if ui
                        .selectable_label(*current_screen == Screen::Skills, "Skills")
                        .clicked()
                    {
                        *current_screen = Screen::Skills;
                    }
                    section_heading(ui, "Agents");
                    if ui
                        .selectable_label(*current_screen == Screen::Agent, "Agent")
                        .clicked()
                    {
                        *current_screen = Screen::Agent;
                    }
                    ui.add_space(12.0);
                    if ui
                        .selectable_label(*current_screen == Screen::Tools, "Tools")
                        .clicked()
                    {
                        *current_screen = Screen::Tools;
                    }
                    section_heading(ui, "System");
                    if ui
                        .selectable_label(*current_screen == Screen::Config, "Config")
                        .clicked()
                    {
                        *current_screen = Screen::Config;
                    }
                    ui.add_space(12.0);
                    if ui
                        .selectable_label(*current_screen == Screen::Gateway, "Gateway")
                        .clicked()
                    {
                        *current_screen = Screen::Gateway;
                    }
                    ui.add_space(12.0);
                    if ui
                        .selectable_label(*current_screen == Screen::Logging, "Logging")
                        .clicked()
                    {
                        *current_screen = Screen::Logging;
                    }
                    section_heading(ui, "Desktop");
                    if ui
                        .selectable_label(*current_screen == Screen::Settings, "Settings")
                        .clicked()
                    {
                        *current_screen = Screen::Settings;
                    }
                });
        });
}
