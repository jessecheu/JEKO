use crate::game::task_trigger::TriggerState;
use egui_macroquad::egui;

pub enum TaskPanelEvent {
    Submit,
}

/// `focus_answer` is a one-shot request (set by pressing E) to put the
/// cursor in the answer field; it's cleared once handled.
pub fn draw(
    ctx: &egui::Context,
    state: &TriggerState,
    input_buffer: &mut String,
    focus_answer: &mut bool,
    in_battle: bool,
) -> Option<TaskPanelEvent> {
    let mut event = None;
    // Beside the road; under the fighters in a battle.
    let (align, offset) = if in_battle {
        (egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -16.0))
    } else {
        (egui::Align2::LEFT_CENTER, egui::vec2(24.0, 0.0))
    };

    match state {
        TriggerState::Idle => {}
        TriggerState::Loading(_) => {
            egui::Window::new("task")
                .title_bar(false)
                .anchor(align, offset)
                .show(ctx, |ui| {
                    ui.label("Asking the tutor for a fresh problem...");
                    ui.spinner();
                });
        }
        TriggerState::Active(task) => {
            egui::Window::new("task")
                .title_bar(false)
                .anchor(align, offset)
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new(&task.problem).size(18.0));
                    ui.label(
                        egui::RichText::new(format!("hint: {}", task.hint))
                            .italics()
                            .weak(),
                    );
                    // Answers are plain numbers, so drop any other typed
                    // characters before the field sees them — otherwise
                    // pressing E (the interact key) while the field has
                    // focus would type an "e" into the answer.
                    ui.input_mut(|i| {
                        for ev in &mut i.events {
                            if let egui::Event::Text(t) = ev {
                                t.retain(|c| c.is_ascii_digit() || matches!(c, '.' | '-'));
                            }
                        }
                    });
                    let mut focused = false;
                    ui.horizontal(|ui| {
                        let resp = ui.text_edit_singleline(input_buffer);
                        if std::mem::take(focus_answer) {
                            resp.request_focus();
                        }
                        focused = resp.has_focus();
                        let enter_pressed =
                            resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if enter_pressed || ui.button("Submit").clicked() {
                            event = Some(TaskPanelEvent::Submit);
                        }
                    });
                    let keys = if focused {
                        "Enter to submit"
                    } else {
                        "Press E to answer"
                    };
                    ui.label(egui::RichText::new(keys).small().weak());
                });
        }
        TriggerState::Resolved { correct, .. } => {
            egui::Window::new("task")
                .title_bar(false)
                .anchor(align, offset)
                .show(ctx, |ui| {
                    let text = if *correct {
                        "Correct!"
                    } else {
                        "Not quite — try the next one."
                    };
                    ui.label(egui::RichText::new(text).size(18.0));
                });
        }
    }

    event
}
