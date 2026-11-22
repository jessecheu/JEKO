use crate::game::{Game, INTERACT_KEY};
use egui_macroquad::egui;

/// The Game Over screen (FR14): the run's stats and how to try again.
/// Returns true when the player clicks "Try again".
pub fn draw(ctx: &egui::Context, game: &Game) -> bool {
    let mut restart = false;
    egui::Window::new("Game Over")
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("You ran out of hearts!")
                        .size(20.0)
                        .strong(),
                );
            });
            ui.separator();
            egui::Grid::new("run_stats").num_columns(2).show(ui, |ui| {
                let rows = [
                    ("Distance", format!("{} m", game.distance_m())),
                    ("Best this session", format!("{} m", game.best_distance_m)),
                    ("Level", game.progression.level.to_string()),
                    ("Best streak", game.progression.best_streak.to_string()),
                    ("Flasks solved", game.run.flasks_solved.to_string()),
                    ("Monsters beaten", game.run.monsters_beaten.to_string()),
                    ("Gears unspent", game.run.gears.to_string()),
                ];
                for (k, v) in rows {
                    ui.label(k);
                    ui.label(egui::RichText::new(v).strong());
                    ui.end_row();
                }
            });
            ui.separator();
            ui.vertical_centered(|ui| {
                if ui.button("Try again").clicked() {
                    restart = true;
                }
                ui.label(format!("or press {:?} / Enter", INTERACT_KEY));
            });
        });
    restart
}
