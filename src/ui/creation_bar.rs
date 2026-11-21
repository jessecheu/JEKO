use crate::llm::activity::{BarView, CircleView, JobState};
use crate::llm::usage::RequestKind;
use egui_macroquad::egui::{self, Color32, Pos2, Stroke};

/// Diameter of one circle.
pub const CIRCLE_D: f32 = 38.0;
/// Height of the bar, with the time under each circle.
pub const BAR_H: f32 = CIRCLE_D + 16.0;
const GAP: f32 = 16.0;

const CREAM: Color32 = Color32::from_rgb(246, 238, 216);
const WOOD_DARK: Color32 = Color32::from_rgb(58, 38, 30);
const MOSS: Color32 = Color32::from_rgb(96, 178, 128);
const LANTERN: Color32 = Color32::from_rgb(255, 179, 71);
const STONE: Color32 = Color32::from_rgb(150, 148, 140);
const VIOLET: Color32 = Color32::from_rgb(140, 100, 190);

/// The creation bar, top centre (FR33): one circle per model call being
/// written. A ring sweeps while it writes, closes when the answer is back, the
/// icon pops, and the time it took shows underneath. A call that failed and was
/// answered from the local bank closes in grey. A view of `BarView` and
/// nothing more (invariant 5). `top` is where the bar starts, so it can sit
/// under the goalpost badge. Returns the bar's height, 0 if nothing is shown.
pub fn draw(ctx: &egui::Context, view: &BarView, top: f32, reduce_flashing: bool) -> f32 {
    if view.circles.is_empty() {
        return 0.0;
    }
    egui::Area::new(egui::Id::new("creation_bar"))
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, top))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                for c in &view.circles {
                    draw_circle(ui, c, reduce_flashing);
                }
                if view.more > 0 {
                    ui.label(
                        egui::RichText::new(format!("+{}", view.more))
                            .strong()
                            .color(CREAM),
                    );
                }
            });
        });
    BAR_H
}

fn draw_circle(ui: &mut egui::Ui, c: &CircleView, reduce_flashing: bool) {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(CIRCLE_D + 12.0, BAR_H), egui::Sense::hover());
    let a = c.alpha;
    let centre = Pos2::new(rect.center().x, rect.top() + CIRCLE_D * 0.5);
    // Pop: springs up and settles when the answer is back.
    let pop = 1.0 + 0.18 * (c.pop * std::f32::consts::PI).sin();
    let r = CIRCLE_D * 0.5 * pop;
    let p = ui.painter();

    let done_col = match c.state {
        JobState::Fallback => STONE,
        _ => MOSS,
    };
    p.circle(
        centre,
        r,
        CREAM.gamma_multiply(a),
        Stroke::new(3.0_f32, WOOD_DARK.gamma_multiply(a)),
    );

    let ring_r = r - 5.0;
    match c.state {
        // A stage ring: an arc that turns while the model writes. It says
        // "working", not "how far along", so it never shows a percentage.
        JobState::Writing => {
            let speed = if reduce_flashing { 0.25 } else { 0.9 };
            let start = c.age * speed * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            arc(
                p,
                centre,
                ring_r,
                start,
                0.3 * std::f32::consts::TAU,
                LANTERN,
                a,
            );
        }
        // Closed.
        _ => arc(
            p,
            centre,
            ring_r,
            -std::f32::consts::FRAC_PI_2,
            std::f32::consts::TAU,
            done_col,
            a,
        ),
    }
    draw_icon(p, c.kind, centre, (ring_r - 4.0).max(0.0), a);

    // A spark as it closes: short rays that fly outwards and fade.
    if let (JobState::Ready, Some(s), false) = (c.state, c.since_end, reduce_flashing) {
        let t = (s / 0.5).clamp(0.0, 1.0);
        if t < 1.0 {
            for i in 0..8 {
                let ang = std::f32::consts::TAU * i as f32 / 8.0;
                let dir = egui::vec2(ang.cos(), ang.sin());
                let from = centre + dir * (r + 2.0 + 8.0 * t);
                let to = centre + dir * (r + 6.0 + 12.0 * t);
                p.line_segment(
                    [from, to],
                    Stroke::new(2.0_f32, LANTERN.gamma_multiply((1.0 - t) * a)),
                );
            }
        }
    }

    // How long it took: the number that shows the speed.
    let text = match (c.state, c.took) {
        (JobState::Fallback, _) => "bank".to_string(),
        (_, Some(s)) => format!("{s:.1} s"),
        _ => String::new(),
    };
    if !text.is_empty() {
        p.text(
            Pos2::new(rect.center().x, rect.bottom() - 2.0),
            egui::Align2::CENTER_BOTTOM,
            text,
            egui::FontId::proportional(12.0),
            CREAM.gamma_multiply(a),
        );
    }

    let what = match c.state {
        JobState::Writing => format!("{}: writing {:.1} s", c.label, c.age),
        JobState::Ready => format!("{}: ready in {:.1} s", c.label, c.took.unwrap_or(0.0)),
        JobState::Fallback => format!("{}: the model was too slow or off, bank used", c.label),
    };
    resp.on_hover_text(format!("{} · {what}", kind_name(c.kind)));
}

fn kind_name(k: RequestKind) -> &'static str {
    match k {
        RequestKind::Question => "Question",
        RequestKind::Prefetch => "Question (ahead)",
        RequestKind::Monster => "Monster",
        RequestKind::Lesson => "Lesson",
    }
}

/// A clockwise arc of `sweep` radians starting at angle `from`.
fn arc(p: &egui::Painter, c: Pos2, r: f32, from: f32, sweep: f32, col: Color32, alpha: f32) {
    if r < 2.0 {
        return;
    }
    let steps = ((sweep / std::f32::consts::TAU * 48.0).ceil() as usize).max(2);
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = from + sweep * i as f32 / steps as f32;
            c + egui::vec2(a.cos(), a.sin()) * r
        })
        .collect();
    p.add(egui::Shape::line(
        pts,
        Stroke::new(3.0_f32, col.gamma_multiply(alpha)),
    ));
}

/// A small flat icon for each kind, inside a circle of radius `s`.
fn draw_icon(p: &egui::Painter, kind: RequestKind, c: Pos2, s: f32, a: f32) {
    if s < 3.0 {
        return;
    }
    let ink = Stroke::new(2.0_f32, WOOD_DARK.gamma_multiply(a));
    match kind {
        // A round flask with a neck.
        RequestKind::Question | RequestKind::Prefetch => {
            p.circle(
                c + egui::vec2(0.0, s * 0.25),
                s * 0.55,
                MOSS.gamma_multiply(a),
                ink,
            );
            let neck = egui::Rect::from_center_size(
                c + egui::vec2(0.0, -s * 0.5),
                egui::vec2(s * 0.4, s * 0.55),
            );
            p.rect(
                neck,
                1.0,
                CREAM.gamma_multiply(a),
                ink,
                egui::StrokeKind::Middle,
            );
        }
        // A little creature: a round body with two eyes and two ears.
        RequestKind::Monster => {
            p.circle(
                c + egui::vec2(0.0, s * 0.1),
                s * 0.62,
                VIOLET.gamma_multiply(a),
                ink,
            );
            for dx in [-0.4_f32, 0.4] {
                p.circle_filled(
                    c + egui::vec2(dx * s, -s * 0.05),
                    s * 0.14,
                    CREAM.gamma_multiply(a),
                );
                p.circle_filled(
                    c + egui::vec2(dx * s, -s * 0.05),
                    s * 0.06,
                    WOOD_DARK.gamma_multiply(a),
                );
                p.line_segment(
                    [
                        c + egui::vec2(dx * s * 1.1, -s * 0.4),
                        c + egui::vec2(dx * s * 1.3, -s * 0.85),
                    ],
                    ink,
                );
            }
        }
        // A scroll.
        RequestKind::Lesson => {
            let r = egui::Rect::from_center_size(c, egui::vec2(s * 1.1, s * 1.3));
            p.rect(
                r,
                2.0,
                CREAM.gamma_multiply(a),
                ink,
                egui::StrokeKind::Middle,
            );
            for i in -1..=1 {
                let y = c.y + i as f32 * s * 0.3;
                p.line_segment(
                    [Pos2::new(c.x - s * 0.35, y), Pos2::new(c.x + s * 0.35, y)],
                    Stroke::new(1.5_f32, WOOD_DARK.gamma_multiply(a)),
                );
            }
        }
    }
}
