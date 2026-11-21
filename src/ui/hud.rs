use crate::progression::Progression;
use egui_macroquad::egui;

/// The core incentive surface: an always-visible progress bar toward the
/// next level, a streak counter with its live bonus multiplier, and an
/// upgrade list showing exactly what's locked and what unlocks it — the
/// student always knows what the next trigger is worth.
pub fn draw(ctx: &egui::Context, progression: &Progression, distance_m: u32, top: f32) {
    egui::Window::new("Progress")
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, top.max(12.0)))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.label(format!("Level {}", progression.level));
            ui.add(
                egui::ProgressBar::new(progression.progress_fraction()).text(format!(
                    "{}/{} XP",
                    progression.xp, progression.xp_to_next_level
                )),
            );
            ui.horizontal(|ui| {
                ui.label(format!("Streak: {}", progression.streak));
                ui.label(format!("(x{:.1} bonus)", progression.streak_multiplier()));
            });
            ui.label(format!("Best streak: {}", progression.best_streak));
            ui.label(format!("Distance: {distance_m} m"));

            ui.separator();
            ui.label("Upgrades:");
            for upgrade in &progression.upgrades {
                let marker = if upgrade.unlocked { "[x]" } else { "[locked]" };
                ui.label(format!(
                    "{marker} {} — {}",
                    upgrade.name, upgrade.description
                ));
            }
        });
}

/// The unlock / event toast, top center. Drawn whatever the HUD setting.
/// `top` is its distance from the top edge: lower than usual while the
/// goalpost tracker (FR31) is up in the same spot.
pub fn draw_toast(ctx: &egui::Context, toast: &Option<(String, f32)>, top: f32) {
    if let Some((msg, _)) = toast {
        egui::Window::new("toast")
            .title_bar(false)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, top))
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(msg).strong().size(18.0));
            });
    }
}
