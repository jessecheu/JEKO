use crate::content::studio::StudioStatus;
use crate::llm::buffer::WaitStats;
use crate::llm::cerebras_client::CerebrasClient;
use crate::llm::usage::{
    RequestKind, RequestUsage, TokenStats, HISTORY_LEN, LIVE_WINDOW, TIMELINE_SPAN,
};
use egui_macroquad::egui;
use std::time::{Duration, Instant};

/// The newest request's bar: deep moss green, about 5:1 against the
/// parchment panel.
const BAR_LATEST: egui::Color32 = egui::Color32::from_rgb(34, 110, 72);
/// Every earlier bar: mid wood, about 3:1 against the parchment panel, so the
/// history stays legible but recedes behind the latest bar.
const BAR_HISTORY: egui::Color32 = egui::Color32::from_rgb(176, 128, 76);
/// A bin or a call that failed: brick red, readable on parchment.
const BAR_FAILED: egui::Color32 = egui::Color32::from_rgb(170, 62, 48);
const GRAPH_H: f32 = 34.0;
const BAR_GAP: f32 = 2.0;
const BAR_RADIUS: u8 = 3;
/// The usage timeline's bins: 30 of 2 s across the last minute.
const TIMELINE_BINS: usize = 30;
/// Height of the timeline's bars; the strip of call spans sits under them.
const TIMELINE_BARS_H: f32 = 30.0;
const SPAN_STRIP_H: f32 = 6.0;
/// How many calls the recent-calls list shows.
const RECENT_ROWS: usize = 4;

/// Live readout of what the cloud model is costing and how fast it's
/// answering: the endpoint; the usage rate right now (tokens per second of
/// wall clock, which falls to zero when nothing is being asked) with a
/// timeline of the last minute and the latest calls; the model's generation
/// speed per request; and session totals. Sits bottom-right, under the
/// Progress panel. Cerebras' speed is what makes a mid-walk problem possible,
/// so it's shown directly.
pub fn draw(
    ctx: &egui::Context,
    client: &CerebrasClient,
    waits: &WaitStats,
    studio: Option<&StudioStatus>,
) {
    let stats = client.token_stats();
    let now = Instant::now();

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

            live_readout(ui, &stats, now);
            timeline(ui, &stats, now);
            recent_calls(ui, &stats, now);
            speed_section(ui, &stats, now);
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

fn speed_section(ui: &mut egui::Ui, stats: &TokenStats, now: Instant) {
    let Some(last) = stats.last() else {
        return;
    };
    let header = format!(
        "Generation speed  {} tok/s",
        fmt_count(last.tokens_per_sec.round() as u64)
    );
    egui::CollapsingHeader::new(egui::RichText::new(header).small())
        .id_salt("token_speed")
        .default_open(false)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(if last.provider_timed {
                    "Per answer: the model's speed, not usage."
                } else {
                    "Per answer, incl. network: not usage."
                })
                .small()
                .weak(),
            );
            graph(ui, stats, now);
        });
}

/// The headline: tokens spent per second over the last few seconds, which
/// is zero whenever the game isn't asking for anything, and what the API is
/// doing right now.
fn live_readout(ui: &mut egui::Ui, stats: &TokenStats, now: Instant) {
    let live = stats.window(now, LIVE_WINDOW);
    let minute = stats.window(now, TIMELINE_SPAN);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(
            egui::RichText::new(fmt_count(live.tokens_per_sec().round() as u64))
                .size(24.0)
                .strong(),
        );
        ui.label("tok/s");
        ui.label(
            egui::RichText::new(format!("last {} s", LIVE_WINDOW.as_secs()))
                .small()
                .weak(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (text, color) = match (stats.in_flight.len(), stats.idle_for(now)) {
                (n @ 1.., _) => (
                    format!("{n} call{} out", if n == 1 { "" } else { "s" }),
                    Some(BAR_LATEST),
                ),
                (_, Some(idle)) => (format!("idle {}", fmt_secs(idle)), None),
                _ => ("no calls yet".to_string(), None),
            };
            let mut rt = egui::RichText::new(text).small();
            rt = match color {
                Some(c) => rt.color(c).strong(),
                None => rt.weak(),
            };
            ui.label(rt);
        });
    });

    let mut line = format!(
        "last min: {} tokens ({} in, {} out) · {} call{}",
        fmt_count(minute.total_tokens()),
        fmt_count(minute.prompt_tokens),
        fmt_count(minute.completion_tokens),
        minute.calls,
        if minute.calls == 1 { "" } else { "s" },
    );
    if minute.failures > 0 {
        line += &format!(", {} failed", minute.failures);
    }
    ui.label(egui::RichText::new(line).small());
}

/// The last minute in 2 s bins, newest on the right: each bar is the tokens
/// of the calls that finished in it. Under the bars, a strip shows when a
/// call was out, so gaps read as the API sitting idle.
fn timeline(ui: &mut egui::Ui, stats: &TokenStats, now: Instant) {
    let (resp, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), TIMELINE_BARS_H + 3.0 + SPAN_STRIP_H),
        egui::Sense::hover(),
    );
    let rect = resp.rect;
    let ink = ui.visuals().widgets.noninteractive.fg_stroke.color;
    let span = TIMELINE_SPAN.as_secs_f64();
    let bins = stats.bins(now, TIMELINE_SPAN, TIMELINE_BINS);
    let peak = bins.iter().map(|b| b.total_tokens()).max().unwrap_or(0);
    let slot = rect.width() / TIMELINE_BINS as f32;
    let bars_bottom = rect.top() + TIMELINE_BARS_H;
    // x for a moment `age` seconds before now.
    let x_at = |age: f64| rect.right() - (age.clamp(0.0, span) / span) as f32 * rect.width();

    let hovered = resp
        .hover_pos()
        .map(|p| (((p.x - rect.left()) / slot).floor() as usize).min(TIMELINE_BINS - 1));

    let top_rounded = egui::CornerRadius {
        nw: BAR_RADIUS,
        ne: BAR_RADIUS,
        sw: 0,
        se: 0,
    };
    for (i, b) in bins.iter().enumerate() {
        let x = rect.left() + i as f32 * slot;
        let (h, color) = if b.total_tokens() > 0 {
            let frac = b.total_tokens() as f32 / peak.max(1) as f32;
            let color = if i == TIMELINE_BINS - 1 {
                BAR_LATEST
            } else {
                BAR_HISTORY
            };
            ((frac * (TIMELINE_BARS_H - 2.0)).max(2.0), color)
        } else if b.failures > 0 {
            (4.0, BAR_FAILED)
        } else {
            continue;
        };
        let r = egui::Rect::from_min_max(
            egui::pos2(x + BAR_GAP / 2.0, bars_bottom - h),
            egui::pos2(x + slot - BAR_GAP / 2.0, bars_bottom),
        );
        painter.rect_filled(r, top_rounded, color);
        if b.failures > 0 && b.total_tokens() > 0 {
            // A failure in a bin that also spent tokens: a red cap.
            painter.rect_filled(
                egui::Rect::from_min_max(r.min, egui::pos2(r.max.x, r.min.y + 3.0)),
                top_rounded,
                BAR_FAILED,
            );
        }
    }
    painter.line_segment(
        [
            egui::pos2(rect.left(), bars_bottom),
            egui::pos2(rect.right(), bars_bottom),
        ],
        egui::Stroke::new(1.0_f32, ink.gamma_multiply(0.35)),
    );

    // When a call was out: finished ones in wood (red if failed), ones still
    // out in moss, growing to "now".
    let strip_top = bars_bottom + 3.0;
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(rect.left(), strip_top),
            egui::pos2(rect.right(), strip_top + SPAN_STRIP_H),
        ),
        2,
        ink.gamma_multiply(0.08),
    );
    let age = |t: Instant| now.saturating_duration_since(t).as_secs_f64();
    let span_rect = |from: f64, to: f64, color: egui::Color32| {
        if from > span && to > span {
            return;
        }
        let (l, r) = (x_at(from), x_at(to));
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(l.min(r - 2.0), strip_top),
                egui::pos2(r, strip_top + SPAN_STRIP_H),
            ),
            2,
            color,
        );
    };
    for c in &stats.timeline {
        let color = if c.failed() { BAR_FAILED } else { BAR_HISTORY };
        span_rect(age(c.started), age(c.finished), color);
    }
    for c in &stats.in_flight {
        span_rect(age(c.started), 0.0, BAR_LATEST);
    }

    if let Some(i) = hovered {
        let b = bins[i];
        let bin_secs = span / TIMELINE_BINS as f64;
        let newer = (TIMELINE_BINS - 1 - i) as f64 * bin_secs;
        let kinds: Vec<String> = RequestKind::ALL
            .into_iter()
            .filter_map(|kind| {
                let n = stats
                    .timeline
                    .iter()
                    .filter(|c| {
                        c.kind == kind && {
                            let a = age(c.finished);
                            a >= newer && a < newer + bin_secs && c.finished <= now
                        }
                    })
                    .count();
                (n > 0).then(|| format!("{n} {}", kind.label()))
            })
            .collect();
        resp.on_hover_ui_at_pointer(|ui| {
            ui.label(
                egui::RichText::new(if newer == 0.0 {
                    format!("Last {} s", bin_secs as u64)
                } else {
                    format!("{}–{} s ago", (newer + bin_secs) as u64, newer as u64)
                })
                .strong(),
            );
            if b.calls + b.failures == 0 {
                ui.label("No calls finished.");
                return;
            }
            let mut calls = format!("{} call{}", b.calls, if b.calls == 1 { "" } else { "s" });
            if b.failures > 0 {
                calls += &format!(", {} failed", b.failures);
            }
            ui.label(calls);
            if !kinds.is_empty() {
                ui.label(kinds.join(" · "));
            }
            ui.label(format!(
                "{} tokens ({} in, {} out)",
                fmt_count(b.total_tokens()),
                fmt_count(b.prompt_tokens),
                fmt_count(b.completion_tokens),
            ));
        });
    }
}

/// The latest calls, newest first: the ones still out, then the ones back,
/// with when they finished, what they were for and what they cost.
fn recent_calls(ui: &mut egui::Ui, stats: &TokenStats, now: Instant) {
    if stats.in_flight.is_empty() && stats.timeline.is_empty() {
        return;
    }
    let small = |s: String| egui::RichText::new(s).small();
    let header = match (stats.in_flight.last(), stats.timeline.back()) {
        (Some(c), _) => format!("Recent calls  ·  {} out now", c.kind.label()),
        (None, Some(c)) => format!(
            "Recent calls  ·  {} {} ago",
            c.kind.label(),
            fmt_secs(now.saturating_duration_since(c.finished))
        ),
        (None, None) => "Recent calls".to_string(),
    };
    egui::CollapsingHeader::new(egui::RichText::new(header).small())
        .id_salt("token_recent_calls")
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("token_recent_grid")
                .num_columns(4)
                .spacing([10.0, 1.0])
                .show(ui, |ui| {
                    let mut rows = 0;
                    for c in stats.in_flight.iter().rev().take(RECENT_ROWS) {
                        ui.label(small("now".into()).color(BAR_LATEST).strong());
                        ui.label(small(c.kind.label().into()));
                        ui.label(small("waiting…".into()).weak());
                        ui.label(small(fmt_ms(now.saturating_duration_since(c.started))));
                        ui.end_row();
                        rows += 1;
                    }
                    for c in stats.timeline.iter().rev().take(RECENT_ROWS - rows) {
                        let ago = now.saturating_duration_since(c.finished);
                        ui.label(small(format!("{} ago", fmt_secs(ago))).weak());
                        ui.label(small(c.kind.label().into()));
                        match c.usage {
                            Some(u) => ui.label(small(format!(
                                "{} tok ({} in, {} out)",
                                fmt_count(u.total_tokens()),
                                fmt_count(u.prompt_tokens),
                                fmt_count(u.completion_tokens),
                            ))),
                            None => ui.label(small("failed".into()).color(BAR_FAILED)),
                        };
                        ui.label(small(fmt_ms(
                            c.finished.saturating_duration_since(c.started),
                        )));
                        ui.end_row();
                    }
                });
        });
}

/// One bar per recent request, newest on the right, scaled to the fastest
/// one in the window: the model's generation speed, which doesn't fall when
/// the game goes quiet. Hovering a bar shows that request's numbers.
fn graph(ui: &mut egui::Ui, stats: &TokenStats, now: Instant) {
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
                "{} · {} ago",
                usage.kind.label(),
                fmt_secs(now.saturating_duration_since(usage.finished)),
            ));
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
    let mut line = format!(
        "Session: {} tokens ({} in, {} out) · {} call{}",
        fmt_count(stats.total_tokens()),
        fmt_count(stats.prompt_tokens),
        fmt_count(stats.completion_tokens),
        stats.requests,
        if stats.requests == 1 { "" } else { "s" },
    );
    if stats.failures > 0 {
        line += &format!(", {} failed", stats.failures);
    }
    ui.label(egui::RichText::new(line).small());
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

/// `0 s`, `42 s`, then `3 min` / `1 h 5 min` for a long quiet spell.
fn fmt_secs(d: Duration) -> String {
    let s = d.as_secs();
    match s {
        0..60 => format!("{s} s"),
        60..3600 => format!("{} min", s / 60),
        _ => format!("{} h {} min", s / 3600, s % 3600 / 60),
    }
}

/// `850 ms`, then `1.2 s` once a call takes a second or more.
fn fmt_ms(d: Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{ms} ms")
    } else {
        format!("{:.1} s", d.as_secs_f64())
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
    use super::{fmt_count, fmt_ms, fmt_secs};
    use std::time::Duration;

    #[test]
    fn durations_read_in_the_unit_that_fits() {
        assert_eq!(fmt_secs(Duration::from_millis(900)), "0 s");
        assert_eq!(fmt_secs(Duration::from_secs(42)), "42 s");
        assert_eq!(fmt_secs(Duration::from_secs(185)), "3 min");
        assert_eq!(fmt_secs(Duration::from_secs(3900)), "1 h 5 min");
        assert_eq!(fmt_ms(Duration::from_millis(850)), "850 ms");
        assert_eq!(fmt_ms(Duration::from_millis(1240)), "1.2 s");
    }

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
