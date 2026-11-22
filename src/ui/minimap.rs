use crate::game::map::{LandmarkKind, MapView};
use egui_macroquad::egui::{self, Align2, Color32, Pos2, Rect, Stroke};

/// Minimap pixels per world pixel of sideways wobble.
const DX_SCALE: f32 = 0.5;
const SIZE: f32 = 120.0;
/// Metres of road shown behind and ahead of the player.
const BEHIND_M: f32 = 25.0;
const AHEAD_M: f32 = 75.0;

/// The always-on minimap (FR27), top right: a small window of the road
/// around the player, drawn from the same [`MapView`] as the full map. The
/// player stays at the centre column; the road scrolls past. Keeps no state.
///
/// Returns the screen y of the panel's bottom edge, so the progress panel
/// can sit beneath it.
pub fn draw(ctx: &egui::Context, view: &MapView) -> f32 {
    let ink = Color32::from_rgb(58, 38, 30);
    let resp = egui::Window::new("minimap")
        .title_bar(false)
        .anchor(Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            let (resp, painter) = ui.allocate_painter(egui::vec2(SIZE, SIZE), egui::Sense::hover());
            let r = resp.rect;
            let painter = painter.with_clip_rect(r);
            painter.rect_filled(r, 2.0, Color32::from_rgb(226, 212, 178));

            let lo = view.player_m - BEHIND_M;
            let y_of = |m: f32| r.bottom() - (m - lo) / (BEHIND_M + AHEAD_M) * SIZE;
            let cx = r.center().x;
            let p = |m: f32, dx: f32| Pos2::new(cx + (dx - view.player_dx) * DX_SCALE, y_of(m));

            // Fog north of the furthest point reached.
            let fog_y = y_of(view.furthest_m);
            if fog_y > r.top() {
                painter.rect_filled(
                    Rect::from_min_max(r.min, Pos2::new(r.right(), fog_y.min(r.bottom()))),
                    0.0,
                    Color32::from_rgb(190, 178, 150),
                );
            }

            let pts: Vec<Pos2> = view.road.iter().map(|&(m, dx)| p(m, dx)).collect();
            if pts.len() > 1 {
                painter.add(egui::Shape::line(
                    pts,
                    Stroke::new(6.0, Color32::from_rgb(176, 128, 82)),
                ));
            }
            for l in &view.landmarks {
                if l.meters < lo || l.meters > lo + BEHIND_M + AHEAD_M {
                    continue;
                }
                let at = p(l.meters, l.dx);
                match l.kind {
                    LandmarkKind::Gate | LandmarkKind::Milestone => {
                        painter.line_segment(
                            [at - egui::vec2(8.0, 0.0), at + egui::vec2(8.0, 0.0)],
                            Stroke::new(1.0, ink),
                        );
                    }
                    LandmarkKind::FlaskFresh => {
                        painter.circle_filled(at, 3.0, Color32::from_rgb(120, 190, 90));
                    }
                    LandmarkKind::FlaskSolved | LandmarkKind::MonsterBeaten => {
                        painter.circle_filled(at, 2.0, Color32::from_rgb(120, 120, 110));
                    }
                    LandmarkKind::FlaskMissed => {
                        painter.circle_filled(at, 2.0, Color32::from_rgb(170, 70, 60));
                    }
                    LandmarkKind::Monster => {
                        painter.circle_filled(at, 3.5, Color32::from_rgb(180, 60, 40));
                    }
                    LandmarkKind::Stop => {
                        painter.circle_filled(at, 3.5, Color32::from_rgb(60, 150, 80));
                    }
                    LandmarkKind::Castle => {
                        painter.rect_filled(
                            Rect::from_center_size(at, egui::vec2(10.0, 8.0)),
                            1.0,
                            Color32::from_rgb(110, 90, 140),
                        );
                    }
                    LandmarkKind::Boss { beaten } => {
                        let c = if beaten {
                            Color32::from_rgb(120, 120, 110)
                        } else {
                            Color32::from_rgb(150, 40, 60)
                        };
                        painter.circle_filled(at, 4.5, c);
                    }
                }
            }
            let you = p(view.player_m, view.player_dx);
            painter.circle_filled(you, 4.0, Color32::from_rgb(255, 179, 71));
            painter.circle_stroke(you, 4.0, Stroke::new(1.5, ink));
            painter.rect_stroke(r, 2.0, Stroke::new(1.0, ink), egui::StrokeKind::Inside);
            ui.label(egui::RichText::new(format!("{} m  (M: map)", view.player_m as u32)).small());
        });
    resp.map_or(0.0, |r| r.response.rect.bottom())
}
