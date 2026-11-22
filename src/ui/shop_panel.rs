use crate::game::health::Health;
use crate::game::shop::{self, Item};
use egui_macroquad::egui;

/// The shop at a safe stop (FR17, FR28): what a gear buys. It keeps no state
/// of its own; it returns the item the player pressed Buy on, and `Game::buy`
/// does the rest. E or Esc (or Close) shuts it, and `Game` handles the keys.
pub fn draw(
    ctx: &egui::Context,
    gears: u32,
    health: &Health,
    note: Option<&str>,
    open: &mut bool,
) -> Option<Item> {
    let mut bought = None;
    egui::Window::new("The Cottage")
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_width(340.0);
            ui.label(
                egui::RichText::new(format!("Your gears: {gears}"))
                    .strong()
                    .size(18.0),
            );
            ui.separator();
            for item in Item::ALL {
                let verdict = shop::check(item, gears, health);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(item.name()).strong());
                        ui.label(egui::RichText::new(item.blurb()).weak());
                        if let Err(why) = verdict {
                            ui.label(egui::RichText::new(why.reason()).small().weak());
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = format!("Buy ({} gears)", item.cost());
                        if ui
                            .add_enabled(verdict.is_ok(), egui::Button::new(label))
                            .clicked()
                        {
                            bought = Some(item);
                        }
                    });
                });
                ui.separator();
            }
            if let Some(note) = note {
                ui.label(egui::RichText::new(note).italics());
            }
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    *open = false;
                }
                ui.label("(E or Esc)");
            });
        });
    bought
}
