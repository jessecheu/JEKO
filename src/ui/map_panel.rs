use crate::game::map::{Landmark, LandmarkKind, MapView};
use egui_macroquad::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke};

/// Map pixels per world pixel of sideways wobble.
const DX_SCALE: f32 = 0.5;
const PANEL_W: f32 = 300.0;
const PANEL_H: f32 = 440.0;

fn icon(painter: &egui::Painter, at: Pos2, l: &Landmark) {
    let ink = Color32::from_rgb(58, 38, 30);
    match l.kind {
        LandmarkKind::Gate => {
            painter.rect_filled(Rect::from_center_size(at, egui::vec2(34.0, 5.0)), 1.0, ink);
        }
        LandmarkKind::Milestone => {
            painter.line_segment(
                [at - egui::vec2(14.0, 0.0), at + egui::vec2(14.0, 0.0)],
                Stroke::new(1.0, ink),
            );
        }
        LandmarkKind::FlaskFresh => {
            painter.circle_filled(at, 4.0, Color32::from_rgb(120, 190, 90));
            painter.circle_stroke(at, 4.0, Stroke::new(1.0, ink));
        }
        LandmarkKind::FlaskSolved => {
            painter.circle_filled(at, 3.0, Color32::from_rgb(120, 120, 110));
        }
        LandmarkKind::FlaskMissed => {
            painter.circle_filled(at, 3.0, Color32::from_rgb(170, 70, 60));
        }
        LandmarkKind::Monster => {
            painter.text(
                at,
                Align2::CENTER_CENTER,
                "!",
                FontId::proportional(14.0),
                Color32::from_rgb(180, 60, 40),
            );
        }
        LandmarkKind::MonsterBeaten => {
            painter.text(
                at,
                Align2::CENTER_CENTER,
                "x",
                FontId::proportional(12.0),
                Color32::from_rgb(120, 120, 110),
            );
        }
        LandmarkKind::Boss { beaten } => {
            let c = if beaten {
                Color32::from_rgb(120, 120, 110)
            } else {
                Color32::from_rgb(150, 40, 60)
            };
            painter.text(
                at,
                Align2::CENTER_CENTER,
                "★",
                FontId::proportional(16.0),
                c,
            );
        }
    }
}

/// The map (FR27), drawn from a [`MapView`]. It keeps no state and returns
/// nothing: M or Esc closes it, and `Game` handles those keys.
pub fn draw(ctx: &egui::Context, view: &MapView) {
    egui::Window::new("Map")
        .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            let (resp, painter) =
                ui.allocate_painter(egui::vec2(PANEL_W, PANEL_H), egui::Sense::hover());
            let r = resp.rect;
            let pad = 28.0;
            let (top, bottom) = (r.top() + pad, r.bottom() - pad);
            let cx = r.center().x;
            let y_of = |m: f32| bottom - (m / view.range_m) * (bottom - top);
            let p = |m: f32, dx: f32| Pos2::new(cx + dx * DX_SCALE, y_of(m));

            let ink = Color32::from_rgb(58, 38, 30);
            // The fog: everything north of the furthest point reached.
            let fog_y = y_of(view.furthest_m);
            painter.rect_filled(
                Rect::from_min_max(r.min, Pos2::new(r.right(), fog_y)),
                0.0,
                Color32::from_rgb(190, 178, 150),
            );
            painter.text(
                Pos2::new(cx, (r.top() + fog_y) / 2.0),
                Align2::CENTER_CENTER,
                "unexplored",
                FontId::proportional(14.0),
                Color32::from_rgb(120, 105, 80),
            );

            let pts: Vec<Pos2> = view.road.iter().map(|&(m, dx)| p(m, dx)).collect();
            if pts.len() > 1 {
                painter.add(egui::Shape::line(
                    pts,
                    Stroke::new(9.0, Color32::from_rgb(176, 128, 82)),
                ));
            }
            for l in &view.landmarks {
                let at = p(l.meters, l.dx);
                icon(&painter, at, l);
                if let Some(m) = l.label {
                    painter.text(
                        at + egui::vec2(-18.0, 0.0),
                        Align2::RIGHT_CENTER,
                        format!("{m} m"),
                        FontId::proportional(11.0),
                        ink,
                    );
                }
                if l.kind == LandmarkKind::Gate {
                    painter.text(
                        at + egui::vec2(20.0, 0.0),
                        Align2::LEFT_CENTER,
                        "Gate",
                        FontId::proportional(11.0),
                        ink,
                    );
                }
            }
            let you = p(view.player_m, view.player_dx);
            painter.circle_filled(you, 6.0, Color32::from_rgb(255, 179, 71));
            painter.circle_stroke(you, 6.0, Stroke::new(2.0, ink));
            painter.text(
                you + egui::vec2(10.0, 0.0),
                Align2::LEFT_CENTER,
                "you",
                FontId::proportional(11.0),
                ink,
            );

            ui.separator();
            ui.label(format!("Furthest: {} m", view.furthest_m as u32));
            ui.label("green dot: flask   !: monster   star: boss   (M or Esc closes)");
        });
}
