use crate::game::field_guide::GuideCard;
use egui_macroquad::egui;

/// The field guide (FR22): a card for every monster, filled in once you've
/// beaten it. Opened with G or from the settings modal; `open` is cleared when
/// the player presses Close.
pub fn draw(ctx: &egui::Context, cards: &[GuideCard], open: &mut bool) {
    let found = cards.iter().filter(|c| c.found).count();
    egui::Window::new("Field Guide")
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(egui::RichText::new(format!("{found} of {} found", cards.len())).weak());
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    for card in cards {
                        if card.found {
                            ui.label(egui::RichText::new(&card.name).strong().size(18.0));
                            if !card.blurb.is_empty() {
                                ui.label(&card.blurb);
                            }
                            if !card.fact.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!("Did you know? {}", card.fact))
                                        .italics()
                                        .weak(),
                                );
                            }
                        } else {
                            ui.label(egui::RichText::new("???").strong().size(18.0));
                            ui.label(egui::RichText::new("Beat it to meet it.").weak());
                        }
                        ui.separator();
                    }
                });
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    *open = false;
                }
                ui.label("(G or Esc)");
            });
        });
}
