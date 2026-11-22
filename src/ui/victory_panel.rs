use crate::game::{world, Game, INTERACT_KEY};
use egui_macroquad::egui;

/// The victory screen (FR27): the player has walked the whole road to the
/// castle gate. Shows the run's stats like the Game Over panel does, and how to
/// walk the road again. Returns true when the player clicks the button.
pub fn draw(ctx: &egui::Context, game: &Game) -> bool {
    let mut restart = false;
    egui::Window::new("The castle")
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("You reached the castle!")
                        .size(20.0)
                        .strong(),
                );
                ui.label("The gate groans open, and somewhere inside the doctor is cackling.");
                ui.label(egui::RichText::new("To be continued...").italics());
            });
            ui.separator();
            egui::Grid::new("victory_stats")
                .num_columns(2)
                .show(ui, |ui| {
                    let rows = [
                        ("Distance", format!("{} m", world::CASTLE_M as u32)),
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
                if ui.button("Walk the road again").clicked() {
                    restart = true;
                }
                ui.label(format!("or press {:?} / Enter", INTERACT_KEY));
            });
        });
    restart
}
