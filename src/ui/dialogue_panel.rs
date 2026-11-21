use crate::game::dialogue::Dialogue;
use egui_macroquad::egui;

/// The NPC dialogue box (FR9): speaker name, the typed-out line, and an
/// E prompt once the line is complete.
pub fn draw(ctx: &egui::Context, dialogue: &Dialogue) {
    egui::Window::new("dialogue")
        .title_bar(false)
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -20.0))
        .resizable(false)
        .fixed_size(egui::vec2(460.0, 70.0))
        .show(ctx, |ui| {
            ui.label(egui::RichText::new(&dialogue.speaker).strong().size(17.0));
            ui.label(egui::RichText::new(dialogue.visible()).size(16.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
                let hint = if dialogue.line_done() {
                    if dialogue.line + 1 < dialogue.lines.len() {
                        "E  next"
                    } else {
                        "E  close"
                    }
                } else {
                    "E  skip"
                };
                ui.label(egui::RichText::new(hint).small().weak());
            });
        });
}
