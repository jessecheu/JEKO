use crate::content::studio::StudioStatus;
use crate::llm::buffer::WaitStats;
use crate::llm::cerebras_client::CerebrasClient;
use crate::llm::usage::{RequestKind, RequestUsage, TokenStats, HISTORY_LEN};
use egui_macroquad::egui;

/// The newest request's bar: deep moss green, about 5:1 against the
/// parchment panel.
const BAR_LATEST: egui::Color32 = egui::Color32::from_rgb(34, 110, 72);
/// Every earlier bar: mid wood, about 3:1 against the parchment panel, so the
/// history stays legible but recedes behind the latest bar.
const BAR_HISTORY: egui::Color32 = egui::Color32::from_rgb(176, 128, 76);
const GRAPH_H: f32 = 52.0;
const BAR_GAP: f32 = 2.0;
const BAR_RADIUS: u8 = 3;

/// Live readout of what the cloud model is costing and how fast it's
/// answering: the endpoint, the latest request's tokens/second with a bar
/// per recent request, and session token totals. Sits bottom-right, under
/// the Progress panel. Cerebras' speed is what makes a mid-walk problem
/// possible, so it's shown directly.
pub fn draw(
    ctx: &egui::Context,
    client: &CerebrasClient,
    waits: &WaitStats,
    studio: Option<&StudioStatus>,
) {
    let stats = client.token_stats();

    egui::Window::new("Token Usage")
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -12.0))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            egui::Grid::new("token_endpoint")
                .num_columns(2)
                .spacing([8.0, 2.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Model").weak());
                    ui.label(egui::RichText::new(client.model()).strong());
                    ui.end_row();
                    ui.label(egui::RichText::new("API").weak());
                    ui.label(egui::RichText::new(client.base_url()).monospace());
                    ui.end_row();
                });

            ui.separator();

            if !client.has_api_key() {
                ui.label("Offline: no CEREBRAS_API_KEY set.");
                ui.label(
                    egui::RichText::new("Problems come from the local fallback bank.")
                        .small()
                        .weak(),
                );
                return;
            }

            speed_readout(ui, &stats);
            graph(ui, &stats);
            totals(ui, &stats);
            by_kind(ui, &stats);
            if let Some(readout) = waits.readout() {
                ui.label(egui::RichText::new(readout).small().weak());
            }
            if let Some(line) = studio.and_then(studio_line) {
                ui.label(egui::RichText::new(line).small().weak());
            }
        });
}

fn speed_readout(ui: &mut egui::Ui, stats: &TokenStats) {
    let (value, caption) = match stats.last() {
        Some(last) => (
            fmt_count(last.tokens_per_sec.round() as u64),
            if last.provider_timed {
                "last request, generation speed"
            } else {
                "last request, incl. network time"
            },
        ),
        None => ("–".to_string(), "Open a flask to make the first request."),
    };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(egui::RichText::new(value).size(24.0).strong());
        ui.label("tok/s");
    });
    ui.label(egui::RichText::new(caption).small().weak());
}

/// One bar per recent request, newest on the right, scaled to the fastest
/// one in the window. Hovering a bar shows that request's numbers.
fn graph(ui: &mut egui::Ui, stats: &TokenStats) {
    let (resp, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), GRAPH_H),
        egui::Sense::hover(),
    );
    let rect = resp.rect;
    let ink = ui.visuals().widgets.noninteractive.fg_stroke.color;

    let slot = rect.width() / HISTORY_LEN as f32;
    let peak = stats.peak_tokens_per_sec();
    // Bars are right-aligned: the newest request always lands in the last slot.
    let first_slot = HISTORY_LEN - stats.history.len();
    let bar_rect = |i: usize, usage: &RequestUsage| {
        let x = rect.left() + (first_slot + i) as f32 * slot;
        let frac = if peak > 0.0 {
            (usage.tokens_per_sec / peak) as f32
        } else {
            0.0
        };
        let h = (frac * (rect.height() - 2.0)).max(2.0);
        egui::Rect::from_min_max(
            egui::pos2(x + BAR_GAP / 2.0, rect.bottom() - h),
            egui::pos2(x + slot - BAR_GAP / 2.0, rect.bottom()),
        )
    };

    let hovered = resp.hover_pos().and_then(|p| {
        let slot_i = ((p.x - rect.left()) / slot).floor() as usize;
        slot_i
            .checked_sub(first_slot)
            .filter(|i| *i < stats.history.len())
    });

    let last_i = stats.history.len().saturating_sub(1);
    let top_rounded = egui::CornerRadius {
        nw: BAR_RADIUS,
        ne: BAR_RADIUS,
        sw: 0,
        se: 0,
    };
    for (i, usage) in stats.history.iter().enumerate() {
        let color = if i == last_i { BAR_LATEST } else { BAR_HISTORY };
        let r = bar_rect(i, usage);
        painter.rect_filled(r, top_rounded, color);
        if hovered == Some(i) {
            painter.rect_stroke(
                r.expand(1.0),
                top_rounded,
                egui::Stroke::new(1.5_f32, ink),
                egui::StrokeKind::Outside,
            );
        }
    }

    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        egui::Stroke::new(1.0_f32, ink.gamma_multiply(0.35)),
    );

    if let Some(i) = hovered {
        let usage = stats.history[i];
        let number = stats.request_number(i);
        resp.on_hover_ui_at_pointer(|ui| {
            ui.label(egui::RichText::new(format!("Request #{number}")).strong());
            ui.label(format!(
                "{} tok/s{}",
                fmt_count(usage.tokens_per_sec.round() as u64),
                if usage.provider_timed {
                    ""
                } else {
                    " (round trip)"
                },
            ));
            ui.label(format!(
                "{} tokens ({} in, {} out)",
                fmt_count(usage.total_tokens()),
                fmt_count(usage.prompt_tokens),
                fmt_count(usage.completion_tokens),
            ));
            ui.label(format!("{} ms round trip", usage.round_trip.as_millis()));
        });
    }

    if !stats.history.is_empty() {
        ui.label(
            egui::RichText::new(format!(
                "avg {} · peak {} tok/s",
                fmt_count(stats.mean_tokens_per_sec().round() as u64),
                fmt_count(peak.round() as u64),
            ))
            .small()
            .weak(),
        );
    }
}

fn totals(ui: &mut egui::Ui, stats: &TokenStats) {
    ui.separator();
    egui::Grid::new("token_totals")
        .num_columns(2)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Tokens").weak());
            ui.label(format!(
                "{} ({} in, {} out)",
                fmt_count(stats.total_tokens()),
                fmt_count(stats.prompt_tokens),
                fmt_count(stats.completion_tokens),
            ));
            ui.end_row();
            ui.label(egui::RichText::new("Requests").weak());
            let mut requests = stats.requests.to_string();
            if stats.failures > 0 {
                requests += &format!(", {} failed", stats.failures);
            }
            ui.label(requests);
            ui.end_row();
        });
}

/// The studio's queue, in a line: "studio: 3 made · 1 queued". Nothing until
/// it has something to say.
fn studio_line(s: &StudioStatus) -> Option<String> {
    if s.out_of_budget {
        return Some(format!("studio: {} made · out of token budget", s.made));
    }
    let mut parts = Vec::new();
    if s.made > 0 {
        parts.push(format!("{} made", s.made));
    }
    let waiting = s.queued + usize::from(s.working);
    if waiting > 0 {
        parts.push(format!("{waiting} queued"));
    }
    if s.rejected + s.failed > 0 {
        parts.push(format!("{} turned down", s.rejected + s.failed));
    }
    (!parts.is_empty()).then(|| format!("studio: {}", parts.join(" · ")))
}

/// Requests per purpose, shown once the game has asked for anything but
/// questions the player was waiting on (look-ahead, monsters, lessons).
fn by_kind(ui: &mut egui::Ui, stats: &TokenStats) {
    let rows: Vec<String> = RequestKind::ALL
        .into_iter()
        .filter_map(|kind| {
            let k = stats.kind(kind);
            (k.requests + k.failures > 0)
                .then(|| format!("{} {} ({})", kind.label(), k.requests, fmt_count(k.tokens)))
        })
        .collect();
    if rows.len() > 1 || stats.kind(RequestKind::Question).requests == 0 && !rows.is_empty() {
        ui.label(egui::RichText::new(rows.join(" · ")).small().weak());
    }
}

/// `1,284`, then `12.9K` / `4.2M` once a number gets too wide for the panel.
fn fmt_count(n: u64) -> String {
    if n < 10_000 {
        return group_thousands(n);
    }
    let k = n as f64 / 1_000.0;
    if k < 999.95 {
        format!("{k:.1}K")
    } else {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    }
}

fn group_thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::fmt_count;

    #[test]
    fn counts_are_grouped_then_compacted() {
        assert_eq!(fmt_count(0), "0");
        assert_eq!(fmt_count(999), "999");
        assert_eq!(fmt_count(1_284), "1,284");
        assert_eq!(fmt_count(9_999), "9,999");
        assert_eq!(fmt_count(12_900), "12.9K");
        assert_eq!(fmt_count(999_949), "999.9K");
        assert_eq!(fmt_count(999_960), "1.0M");
        assert_eq!(fmt_count(4_200_000), "4.2M");
    }
}
