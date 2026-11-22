//! Token accounting for calls to the cloud model, read each frame by the
//! HUD's token panel (`src/ui/token_panel.rs`).
//!
//! Two kinds of number live here, and they answer different questions:
//!
//! - **Generation speed** (`RequestUsage::tokens_per_sec`): how fast the model
//!   wrote one answer. It's a property of the model, so it stays put while the
//!   game is idle.
//! - **Usage rate** (`TokenStats::window`): how many tokens the game spent over
//!   the last few seconds of wall clock. It drops to zero when nothing is
//!   being asked.

use serde_json::Value;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// How many recent requests the token panel's speed graph keeps.
pub const HISTORY_LEN: usize = 24;
/// How far back the usage timeline graph reaches, and the "last minute" window
/// (the unit Cerebras' rate limits are counted in).
pub const TIMELINE_SPAN: Duration = Duration::from_secs(60);
/// The window behind the panel's live usage rate.
pub const LIVE_WINDOW: Duration = Duration::from_secs(10);
/// How many finished calls the timeline keeps, whatever their age.
pub const TIMELINE_CAP: usize = 128;

/// What a model call was for. Tagged on every `RequestUsage`, so the token
/// panel can split the session's throughput by purpose (FR21, FR32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestKind {
    /// A question the player is waiting on, or about to.
    #[default]
    Question,
    /// A question asked ahead of need, to fill the question buffer (FR32).
    Prefetch,
    /// A monster designed by the studio (FR21).
    Monster,
    /// A lesson pack from the studio (FR22).
    Lesson,
    /// A line of the villain's, or any other spoken line (FR23, FR33).
    Line,
}

impl RequestKind {
    pub const ALL: [RequestKind; 5] = [
        RequestKind::Question,
        RequestKind::Prefetch,
        RequestKind::Monster,
        RequestKind::Lesson,
        RequestKind::Line,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RequestKind::Question => "question",
            RequestKind::Prefetch => "prefetch",
            RequestKind::Monster => "monster",
            RequestKind::Lesson => "lesson",
            RequestKind::Line => "line",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// What one chat-completions call cost and how fast it generated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RequestUsage {
    pub kind: RequestKind,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    /// Output tokens per second.
    pub tokens_per_sec: f64,
    /// True when `tokens_per_sec` comes from the provider's own generation
    /// timing (Cerebras' `time_info.completion_time`). False when it falls
    /// back to the HTTP round trip, which also counts network and queueing
    /// and so reads lower than the model's real speed.
    pub provider_timed: bool,
    pub round_trip: Duration,
    /// When the answer came back.
    pub finished: Instant,
}

impl RequestUsage {
    /// Reads the OpenAI-style `usage` block of a chat-completions response
    /// for a call sent at `started` and answered at `finished`. `None` if the
    /// provider didn't send one.
    pub fn from_response(
        resp: &Value,
        started: Instant,
        finished: Instant,
        kind: RequestKind,
    ) -> Option<Self> {
        let round_trip = finished.saturating_duration_since(started);
        let usage = &resp["usage"];
        let completion_tokens = usage["completion_tokens"].as_u64()?;
        let prompt_tokens = usage["prompt_tokens"].as_u64().unwrap_or(0);

        let provider_secs = resp["time_info"]["completion_time"]
            .as_f64()
            .filter(|s| *s > 0.0);
        let secs = provider_secs.unwrap_or(round_trip.as_secs_f64());
        let tokens_per_sec = if secs > 0.0 {
            completion_tokens as f64 / secs
        } else {
            0.0
        };

        Some(Self {
            kind,
            prompt_tokens,
            completion_tokens,
            tokens_per_sec,
            provider_timed: provider_secs.is_some(),
            round_trip,
            finished,
        })
    }

    pub fn total_tokens(&self) -> u64 {
        self.prompt_tokens + self.completion_tokens
    }

    /// When the call was sent.
    pub fn started(&self) -> Instant {
        self.finished
            .checked_sub(self.round_trip)
            .unwrap_or(self.finished)
    }
}

/// One finished call on the usage timeline: answered, or failed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CallRecord {
    pub kind: RequestKind,
    pub started: Instant,
    pub finished: Instant,
    /// `None` when the call failed (timeout, HTTP error, bad body).
    pub usage: Option<RequestUsage>,
}

impl CallRecord {
    pub fn failed(&self) -> bool {
        self.usage.is_none()
    }

    pub fn tokens(&self) -> u64 {
        self.usage.map_or(0, |u| u.total_tokens())
    }
}

/// A call that has been sent and hasn't come back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InFlight {
    pub id: u64,
    pub kind: RequestKind,
    pub started: Instant,
}

/// What the calls that finished inside a stretch of wall clock added up to.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UsageWindow {
    pub calls: u32,
    pub failures: u32,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    /// The stretch's length.
    pub secs: f64,
}

impl UsageWindow {
    fn add(&mut self, c: &CallRecord) {
        match c.usage {
            Some(u) => {
                self.calls += 1;
                self.prompt_tokens += u.prompt_tokens;
                self.completion_tokens += u.completion_tokens;
            }
            None => self.failures += 1,
        }
    }

    pub fn total_tokens(&self) -> u64 {
        self.prompt_tokens + self.completion_tokens
    }

    /// Tokens spent per second of wall clock (in and out), averaged over the
    /// whole stretch. Zero when nothing was asked.
    pub fn tokens_per_sec(&self) -> f64 {
        if self.secs > 0.0 {
            self.total_tokens() as f64 / self.secs
        } else {
            0.0
        }
    }
}

/// Running totals for the session, the last `HISTORY_LEN` requests for the
/// speed graph, and a timeline of recent calls (with the ones still out) for
/// the usage graph.
#[derive(Debug, Clone, Default)]
pub struct TokenStats {
    /// Requests that came back with a `usage` block.
    pub requests: u32,
    /// Requests that errored (timeout, HTTP error, bad body).
    pub failures: u32,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    /// Oldest first.
    pub history: VecDeque<RequestUsage>,
    /// The last `TIMELINE_CAP` finished calls, failures included, in the
    /// order they finished.
    pub timeline: VecDeque<CallRecord>,
    /// Calls sent and not back yet, oldest first.
    pub in_flight: Vec<InFlight>,
    /// When the newest call finished, however long ago.
    pub last_finished: Option<Instant>,
    next_id: u64,
    /// Requests and tokens per `RequestKind`, indexed by it.
    by_kind: [KindTotals; 5],
}

/// One kind's share of the session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KindTotals {
    pub requests: u32,
    pub failures: u32,
    pub tokens: u64,
}

impl TokenStats {
    /// Notes a call going out. Pair it with `end` once it's back, whatever
    /// the outcome.
    pub fn begin(&mut self, kind: RequestKind, started: Instant) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.in_flight.push(InFlight { id, kind, started });
        id
    }

    /// Takes a call off the in-flight list. Ending one twice does nothing.
    pub fn end(&mut self, id: u64) {
        self.in_flight.retain(|c| c.id != id);
    }

    pub fn record(&mut self, usage: RequestUsage) {
        self.requests += 1;
        self.prompt_tokens += usage.prompt_tokens;
        self.completion_tokens += usage.completion_tokens;
        let k = &mut self.by_kind[usage.kind.index()];
        k.requests += 1;
        k.tokens += usage.total_tokens();
        if self.history.len() == HISTORY_LEN {
            self.history.pop_front();
        }
        self.history.push_back(usage);
        self.push_call(CallRecord {
            kind: usage.kind,
            started: usage.started(),
            finished: usage.finished,
            usage: Some(usage),
        });
    }

    pub fn record_failure(&mut self, kind: RequestKind, started: Instant, finished: Instant) {
        self.failures += 1;
        self.by_kind[kind.index()].failures += 1;
        self.push_call(CallRecord {
            kind,
            started,
            finished,
            usage: None,
        });
    }

    fn push_call(&mut self, call: CallRecord) {
        if self.timeline.len() == TIMELINE_CAP {
            self.timeline.pop_front();
        }
        self.timeline.push_back(call);
        self.last_finished = Some(
            self.last_finished
                .map_or(call.finished, |t| t.max(call.finished)),
        );
    }

    /// The session's requests and tokens for one kind of call.
    pub fn kind(&self, kind: RequestKind) -> KindTotals {
        self.by_kind[kind.index()]
    }

    pub fn last(&self) -> Option<&RequestUsage> {
        self.history.back()
    }

    pub fn total_tokens(&self) -> u64 {
        self.prompt_tokens + self.completion_tokens
    }

    /// Fastest request still in the graph's window.
    pub fn peak_tokens_per_sec(&self) -> f64 {
        self.history
            .iter()
            .map(|u| u.tokens_per_sec)
            .fold(0.0, f64::max)
    }

    /// Mean speed over the graph's window.
    pub fn mean_tokens_per_sec(&self) -> f64 {
        if self.history.is_empty() {
            return 0.0;
        }
        self.history.iter().map(|u| u.tokens_per_sec).sum::<f64>() / self.history.len() as f64
    }

    /// 1-based request number of `history[i]`, for the graph's tooltip.
    pub fn request_number(&self, i: usize) -> u32 {
        self.requests - self.history.len() as u32 + i as u32 + 1
    }

    /// What the calls that finished in the last `window` before `now` spent.
    /// A call still out counts once it lands.
    pub fn window(&self, now: Instant, window: Duration) -> UsageWindow {
        let mut w = UsageWindow {
            secs: window.as_secs_f64(),
            ..Default::default()
        };
        for c in self
            .timeline
            .iter()
            .filter(|c| within(c.finished, now, window))
        {
            w.add(c);
        }
        w
    }

    /// The last `span` before `now` cut into `n` equal bins, oldest first,
    /// each holding the calls that finished in it.
    pub fn bins(&self, now: Instant, span: Duration, n: usize) -> Vec<UsageWindow> {
        let bin_secs = span.as_secs_f64() / n.max(1) as f64;
        let mut bins = vec![
            UsageWindow {
                secs: bin_secs,
                ..Default::default()
            };
            n
        ];
        for c in &self.timeline {
            if !within(c.finished, now, span) {
                continue;
            }
            let age = now.saturating_duration_since(c.finished).as_secs_f64();
            let back = ((age / bin_secs) as usize).min(n - 1);
            bins[n - 1 - back].add(c);
        }
        bins
    }

    /// How long since the last call finished, while none is out. `None`
    /// while a call is out, or before the first one.
    pub fn idle_for(&self, now: Instant) -> Option<Duration> {
        if !self.in_flight.is_empty() {
            return None;
        }
        self.last_finished.map(|t| now.saturating_duration_since(t))
    }
}

/// True when `t` falls in the `window` that ends at `now`.
fn within(t: Instant, now: Instant, window: Duration) -> bool {
    t <= now && now.duration_since(t) < window
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample(tps: f64) -> RequestUsage {
        sample_at(tps, Instant::now())
    }

    fn sample_at(tps: f64, finished: Instant) -> RequestUsage {
        RequestUsage {
            kind: RequestKind::Question,
            prompt_tokens: 100,
            completion_tokens: 50,
            tokens_per_sec: tps,
            provider_timed: true,
            round_trip: Duration::from_millis(200),
            finished,
        }
    }

    /// `from_response` for a call that took `ms`.
    fn parse(resp: &Value, ms: u64) -> Option<RequestUsage> {
        let t = Instant::now();
        RequestUsage::from_response(
            resp,
            t,
            t + Duration::from_millis(ms),
            RequestKind::Question,
        )
    }

    #[test]
    fn speed_comes_from_provider_timing_when_present() {
        let resp = json!({
            "usage": {"prompt_tokens": 120, "completion_tokens": 200, "total_tokens": 320},
            "time_info": {"completion_time": 0.1, "total_time": 0.15},
        });
        let u = parse(&resp, 400).unwrap();
        assert_eq!(u.prompt_tokens, 120);
        assert_eq!(u.completion_tokens, 200);
        assert_eq!(u.total_tokens(), 320);
        assert!((u.tokens_per_sec - 2000.0).abs() < 1e-6);
        assert!(u.provider_timed);
    }

    #[test]
    fn speed_falls_back_to_round_trip() {
        let resp = json!({"usage": {"prompt_tokens": 10, "completion_tokens": 100}});
        let u = parse(&resp, 500).unwrap();
        assert!((u.tokens_per_sec - 200.0).abs() < 1e-6);
        assert!(!u.provider_timed);
    }

    #[test]
    fn zero_provider_time_is_ignored() {
        let resp = json!({
            "usage": {"prompt_tokens": 10, "completion_tokens": 100},
            "time_info": {"completion_time": 0.0},
        });
        let u = parse(&resp, 1000).unwrap();
        assert!((u.tokens_per_sec - 100.0).abs() < 1e-6);
        assert!(!u.provider_timed);
    }

    #[test]
    fn no_usage_block_means_no_sample() {
        let resp = json!({"choices": [{"message": {"content": "{}"}}]});
        assert!(parse(&resp, 100).is_none());
    }

    #[test]
    fn history_is_capped_but_totals_keep_counting() {
        let mut stats = TokenStats::default();
        for i in 0..HISTORY_LEN + 5 {
            stats.record(sample(i as f64));
        }
        assert_eq!(stats.history.len(), HISTORY_LEN);
        assert_eq!(stats.requests as usize, HISTORY_LEN + 5);
        assert_eq!(stats.total_tokens(), 150 * (HISTORY_LEN as u64 + 5));
        assert_eq!(
            stats.last().unwrap().tokens_per_sec,
            (HISTORY_LEN + 4) as f64
        );
        assert_eq!(stats.peak_tokens_per_sec(), (HISTORY_LEN + 4) as f64);
        // The oldest bar left in the window is request #6.
        assert_eq!(stats.request_number(0), 6);
        assert_eq!(
            stats.request_number(HISTORY_LEN - 1) as usize,
            HISTORY_LEN + 5
        );
    }

    #[test]
    fn totals_are_split_by_kind() {
        let mut stats = TokenStats::default();
        stats.record(sample(10.0));
        stats.record(RequestUsage {
            kind: RequestKind::Monster,
            ..sample(10.0)
        });
        let t = Instant::now();
        stats.record_failure(RequestKind::Prefetch, t, t);
        assert_eq!(stats.kind(RequestKind::Question).requests, 1);
        assert_eq!(stats.kind(RequestKind::Monster).tokens, 150);
        assert_eq!(stats.kind(RequestKind::Prefetch).failures, 1);
        assert_eq!(stats.kind(RequestKind::Lesson), KindTotals::default());
        assert_eq!(stats.requests, 2);
        assert_eq!(stats.failures, 1);
    }

    #[test]
    fn usage_rate_falls_to_zero_once_calls_stop() {
        let t0 = Instant::now();
        let mut stats = TokenStats::default();
        stats.record(sample_at(400.0, t0));
        stats.record(sample_at(400.0, t0 + Duration::from_secs(1)));
        // Two 150-token calls in the last 10 s: 30 tok/s.
        let live = stats.window(t0 + Duration::from_secs(2), LIVE_WINDOW);
        assert_eq!(live.calls, 2);
        assert!((live.tokens_per_sec() - 30.0).abs() < 1e-6);
        // Idle for a while: the rate is zero, though the last call's
        // generation speed is still 400.
        let later = t0 + Duration::from_secs(30);
        assert_eq!(stats.window(later, LIVE_WINDOW).tokens_per_sec(), 0.0);
        assert_eq!(stats.window(later, TIMELINE_SPAN).calls, 2);
        assert_eq!(stats.last().unwrap().tokens_per_sec, 400.0);
        assert_eq!(stats.idle_for(later), Some(Duration::from_secs(29)));
    }

    #[test]
    fn bins_place_calls_by_when_they_finished() {
        let t0 = Instant::now();
        let mut stats = TokenStats::default();
        stats.record(sample_at(1.0, t0));
        stats.record_failure(RequestKind::Monster, t0, t0 + Duration::from_secs(5));
        stats.record(sample_at(1.0, t0 + Duration::from_secs(59)));
        let now = t0 + Duration::from_secs(60);
        let bins = stats.bins(now, TIMELINE_SPAN, 30);
        assert_eq!(bins.len(), 30);
        // The first call is exactly 60 s old: out of the window.
        assert_eq!(bins.iter().map(|b| b.calls).sum::<u32>(), 1);
        assert_eq!(bins[29].calls, 1);
        assert_eq!(bins[29].total_tokens(), 150);
        // 55 s ago, in 2 s bins: the 28th from the right.
        assert_eq!(bins[2].failures, 1);
        assert_eq!(stats.window(now, TIMELINE_SPAN).failures, 1);
    }

    #[test]
    fn calls_in_flight_are_tracked_until_they_end() {
        let t0 = Instant::now();
        let mut stats = TokenStats::default();
        assert_eq!(stats.idle_for(t0), None);
        let a = stats.begin(RequestKind::Question, t0);
        let b = stats.begin(RequestKind::Lesson, t0);
        assert_eq!(stats.in_flight.len(), 2);
        stats.end(a);
        stats.end(a);
        assert_eq!(stats.in_flight.len(), 1);
        assert_eq!(stats.in_flight[0].kind, RequestKind::Lesson);
        stats.record(sample_at(1.0, t0));
        // Still busy while one is out.
        assert_eq!(stats.idle_for(t0 + Duration::from_secs(3)), None);
        stats.end(b);
        assert_eq!(
            stats.idle_for(t0 + Duration::from_secs(3)),
            Some(Duration::from_secs(3))
        );
    }

    #[test]
    fn the_timeline_is_capped() {
        let t0 = Instant::now();
        let mut stats = TokenStats::default();
        for _ in 0..TIMELINE_CAP + 3 {
            stats.record(sample_at(1.0, t0));
        }
        assert_eq!(stats.timeline.len(), TIMELINE_CAP);
        assert_eq!(stats.requests as usize, TIMELINE_CAP + 3);
    }
}
