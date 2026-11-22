use crate::game::goalpost::TrackerView;
use crate::game::world::GoalKind;
use egui_macroquad::egui::{self, Color32, Pos2, Stroke};

/// Height of the badge, and the diameter of its circle.
pub const BADGE_D: f32 = 40.0;
/// Where the toast goes while the badge is up, so the two never overlap:
/// just below it.
pub const TOAST_BELOW_BADGE: f32 = 12.0 + BADGE_D + 14.0;

const CREAM: Color32 = Color32::from_rgb(246, 238, 216);
const WOOD_DARK: Color32 = Color32::from_rgb(58, 38, 30);
const MOSS: Color32 = Color32::from_rgb(96, 178, 128);
const LANTERN: Color32 = Color32::from_rgb(255, 179, 71);
const STONE: Color32 = Color32::from_rgb(170, 168, 160);

/// The goalpost tracker, top centre (FR31): a circle holding the goalpost's
/// icon, which unrolls into a pill with its name and the metres left. A ring
/// around the circle fills as the player closes in. A view of `TrackerView`
/// and nothing more (invariant 5).
pub fn draw(ctx: &egui::Context, v: &TrackerView) {
    egui::Area::new(egui::Id::new("goalpost"))
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 12.0))
        .interactable(false)
        .show(ctx, |ui| {
            let font = egui::FontId::proportional(17.0);
            let text = if v.arrived {
                "Now!".to_string()
            } else {
                format!("{} · {} m", v.label, v.meters)
            };
            let text_w = ui
                .painter()
                .layout_no_wrap(text.clone(), font.clone(), WOOD_DARK)
                .size()
                .x;
            let full_w = BADGE_D + 10.0 + text_w + 18.0;
            let open = ease_out(v.open);
            let w = BADGE_D + (full_w - BADGE_D) * open;
            // The width of the area changes as it unrolls, so the badge stays
            // centred while it grows both ways.
            let (rect, _) = ui.allocate_exact_size(egui::vec2(w, BADGE_D), egui::Sense::hover());
            let painter = ui.painter();

            // Pop: the circle springs up from nothing and overshoots a touch.
            let pop = ease_out_back(v.pop);
            let pulse = if v.pulse {
                1.0 + 0.07 * (v.clock * 9.0).sin().abs()
            } else {
                1.0
            };
            let r = BADGE_D * 0.5 * pop * pulse;
            let circle_c = Pos2::new(rect.left() + BADGE_D * 0.5, rect.center().y);

            // The pill (the circle itself while closed).
            let pill = egui::Rect::from_min_max(
                Pos2::new(circle_c.x - r, circle_c.y - r),
                Pos2::new(
                    (rect.right() - (BADGE_D * 0.5 - r)).max(circle_c.x + r),
                    circle_c.y + r,
                ),
            );
            painter.rect(
                pill,
                egui::CornerRadius::same(r.clamp(0.0, 255.0) as u8),
                CREAM,
                Stroke::new(3.0_f32, WOOD_DARK),
                egui::StrokeKind::Middle,
            );

            // The progress ring, clockwise from the top.
            let ring_r = (r - 5.0).max(0.0);
            let ring_col = if v.arrived || v.pulse { LANTERN } else { MOSS };
            if v.ring > 0.0 && ring_r > 2.0 {
                let steps = ((v.ring * 48.0).ceil() as usize).max(2);
                let pts: Vec<Pos2> = (0..=steps)
                    .map(|i| {
                        let a = -std::f32::consts::FRAC_PI_2
                            + std::f32::consts::TAU * v.ring * i as f32 / steps as f32;
                        circle_c + egui::vec2(a.cos(), a.sin()) * ring_r
                    })
                    .collect();
                painter.add(egui::Shape::line(pts, Stroke::new(3.0_f32, ring_col)));
            }
            draw_icon(painter, v.kind, circle_c, (ring_r - 4.0).max(0.0));

            // The label fades in once the pill is mostly open, and is clipped
            // to it while it unrolls.
            let alpha = ((v.open - 0.55) / 0.45).clamp(0.0, 1.0);
            if alpha > 0.0 {
                let col = WOOD_DARK.gamma_multiply(alpha);
                painter.with_clip_rect(pill.shrink(3.0)).text(
                    Pos2::new(circle_c.x + BADGE_D * 0.5 + 4.0, pill.center().y),
                    egui::Align2::LEFT_CENTER,
                    text,
                    font,
                    col,
                );
            }
        });
}

fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

/// Ease-out with a small overshoot, for the circle's pop.
fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// A small flat icon for each kind of goalpost, inside a circle of radius
/// `s` around `c`.
fn draw_icon(painter: &egui::Painter, kind: GoalKind, c: Pos2, s: f32) {
    if s < 3.0 {
        return;
    }
    let ink = Stroke::new(2.0_f32, WOOD_DARK);
    match kind {
        // A distance stone.
        GoalKind::Milestone => {
            let r = egui::Rect::from_center_size(
                c + egui::vec2(0.0, 1.0),
                egui::vec2(s * 1.0, s * 1.3),
            );
            painter.rect(
                r,
                egui::CornerRadius::same((s * 0.35) as u8),
                STONE,
                ink,
                egui::StrokeKind::Middle,
            );
        }
        // A round flask with a neck.
        GoalKind::Flask => {
            painter.circle(c + egui::vec2(0.0, s * 0.25), s * 0.55, MOSS, ink);
            let neck = egui::Rect::from_center_size(
                c + egui::vec2(0.0, -s * 0.5),
                egui::vec2(s * 0.4, s * 0.55),
            );
            painter.rect(neck, 1.0, CREAM, ink, egui::StrokeKind::Middle);
        }
        // Three fern fronds fanning up.
        GoalKind::Patch => {
            let base = c + egui::vec2(0.0, s * 0.7);
            for dx in [-0.7_f32, 0.0, 0.7] {
                let tip = c + egui::vec2(dx * s, -s * 0.7 + dx.abs() * s * 0.3);
                painter.line_segment(
                    [base, tip],
                    Stroke::new(2.5_f32, Color32::from_rgb(52, 120, 84)),
                );
            }
        }
        // A rune ward across the road: a bold "!".
        GoalKind::Blocker => {
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "!",
                egui::FontId::proportional(s * 1.9),
                Color32::from_rgb(150, 50, 60),
            );
        }
        // A little house, for a safe stop.
        GoalKind::Stop => {
            let l = s * 0.8;
            let walls = egui::Rect::from_min_max(
                c + egui::vec2(-l * 0.8, -l * 0.1),
                c + egui::vec2(l * 0.8, l * 0.8),
            );
            painter.rect(walls, 1.0, CREAM, ink, egui::StrokeKind::Middle);
            let roof = vec![
                c + egui::vec2(-l, -l * 0.1),
                c + egui::vec2(0.0, -l * 0.9),
                c + egui::vec2(l, -l * 0.1),
            ];
            painter.add(egui::Shape::convex_polygon(roof, LANTERN, ink));
        }
        // A signpost, for a region's border (FR27).
        GoalKind::Region => {
            let post = egui::Rect::from_center_size(
                c + egui::vec2(0.0, s * 0.25),
                egui::vec2(s * 0.3, s * 1.5),
            );
            painter.rect(post, 1.0, WOOD_DARK, ink, egui::StrokeKind::Middle);
            let board = egui::Rect::from_center_size(
                c + egui::vec2(0.0, -s * 0.35),
                egui::vec2(s * 1.7, s * 0.8),
            );
            painter.rect(board, 2.0, CREAM, ink, egui::StrokeKind::Middle);
        }
        // A keep with two towers, for the castle (FR27).
        GoalKind::Castle => {
            let l = s * 0.85;
            let keep = egui::Rect::from_min_max(
                c + egui::vec2(-l * 0.55, -l * 0.3),
                c + egui::vec2(l * 0.55, l * 0.8),
            );
            painter.rect(keep, 1.0, STONE, ink, egui::StrokeKind::Middle);
            for side in [-1.0_f32, 1.0] {
                let tower = egui::Rect::from_min_max(
                    c + egui::vec2(side * l * 0.95 - l * 0.28, -l * 0.8),
                    c + egui::vec2(side * l * 0.95 + l * 0.28, l * 0.8),
                );
                painter.rect(tower, 1.0, STONE, ink, egui::StrokeKind::Middle);
            }
            let gate = egui::Rect::from_min_max(
                c + egui::vec2(-l * 0.18, l * 0.1),
                c + egui::vec2(l * 0.18, l * 0.8),
            );
            painter.rect_filled(gate, 1.0, WOOD_DARK);
        }
        // A crown, for the boss.
        GoalKind::Titan => {
            let l = s * 0.85;
            let pts = vec![
                c + egui::vec2(-l, l * 0.6),
                c + egui::vec2(-l, -l * 0.4),
                c + egui::vec2(-l * 0.5, l * 0.1),
                c + egui::vec2(0.0, -l * 0.7),
                c + egui::vec2(l * 0.5, l * 0.1),
                c + egui::vec2(l, -l * 0.4),
                c + egui::vec2(l, l * 0.6),
            ];
            painter.add(egui::Shape::convex_polygon(pts, LANTERN, ink));
        }
    }
}
