use crate::game::Settings;
use egui_macroquad::egui;

/// Every key the game listens to, for the read-only reference.
const CONTROLS: &[(&str, &str)] = &[
    ("W A S D / arrows", "Walk"),
    ("Shift (hold)", "Sprint"),
    ("E", "Open a flask / jump to the answer field"),
    ("Enter", "Submit an answer"),
    ("Space", "Jump"),
    ("Ctrl", "Roll"),
    ("F", "Shoot an arrow"),
    ("P", "Pause"),
    ("O / Esc", "Settings"),
];

/// The settings modal (FR7). Changes apply immediately; `open` is cleared
/// when the player presses Close.
pub fn draw(ctx: &egui::Context, settings: &mut Settings, open: &mut bool) {
    egui::Window::new("Settings")
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.heading("Display");
            ui.checkbox(&mut settings.show_hud, "Show progress panel");
            ui.checkbox(&mut settings.show_token_panel, "Show token usage panel");
            ui.checkbox(
                &mut settings.reduce_flashing,
                "Reduce flashing (steady tint instead of flicker)",
            );

            ui.separator();
            ui.heading("Controls");
            egui::Grid::new("controls").num_columns(2).show(ui, |ui| {
                for (key, action) in CONTROLS {
                    ui.label(egui::RichText::new(*key).strong());
                    ui.label(*action);
                    ui.end_row();
                }
            });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    *open = false;
                }
                ui.label("(O or Esc)");
            });
        });
}
