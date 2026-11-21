//! Token accounting for calls to the cloud model, read each frame by the
//! HUD's token panel (`src/ui/token_panel.rs`).

use serde_json::Value;
use std::collections::VecDeque;
use std::time::Duration;

/// How many recent requests the token panel's graph keeps.
pub const HISTORY_LEN: usize = 24;

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
}

impl RequestKind {
    pub const ALL: [RequestKind; 4] = [
        RequestKind::Question,
        RequestKind::Prefetch,
        RequestKind::Monster,
        RequestKind::Lesson,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RequestKind::Question => "question",
            RequestKind::Prefetch => "prefetch",
            RequestKind::Monster => "monster",
            RequestKind::Lesson => "lesson",
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
}

impl RequestUsage {
    /// Reads the OpenAI-style `usage` block of a chat-completions response.
    /// `None` if the provider didn't send one.
    pub fn from_response(resp: &Value, round_trip: Duration, kind: RequestKind) -> Option<Self> {
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
        })
    }

    pub fn total_tokens(&self) -> u64 {
        self.prompt_tokens + self.completion_tokens
    }
}

/// Running totals for the session, plus the last `HISTORY_LEN` requests
/// for the graph.
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
    /// Requests and tokens per `RequestKind`, indexed by it.
    by_kind: [KindTotals; 4],
}

/// One kind's share of the session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KindTotals {
    pub requests: u32,
    pub failures: u32,
    pub tokens: u64,
}

impl TokenStats {
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
    }

    pub fn record_failure(&mut self, kind: RequestKind) {
        self.failures += 1;
        self.by_kind[kind.index()].failures += 1;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample(tps: f64) -> RequestUsage {
        RequestUsage {
            kind: RequestKind::Question,
            prompt_tokens: 100,
            completion_tokens: 50,
            tokens_per_sec: tps,
            provider_timed: true,
            round_trip: Duration::from_millis(200),
        }
    }

    #[test]
    fn speed_comes_from_provider_timing_when_present() {
        let resp = json!({
            "usage": {"prompt_tokens": 120, "completion_tokens": 200, "total_tokens": 320},
            "time_info": {"completion_time": 0.1, "total_time": 0.15},
        });
        let u =
            RequestUsage::from_response(&resp, Duration::from_millis(400), RequestKind::Question)
                .unwrap();
        assert_eq!(u.prompt_tokens, 120);
        assert_eq!(u.completion_tokens, 200);
        assert_eq!(u.total_tokens(), 320);
        assert!((u.tokens_per_sec - 2000.0).abs() < 1e-6);
        assert!(u.provider_timed);
    }

    #[test]
    fn speed_falls_back_to_round_trip() {
        let resp = json!({"usage": {"prompt_tokens": 10, "completion_tokens": 100}});
        let u =
            RequestUsage::from_response(&resp, Duration::from_millis(500), RequestKind::Question)
                .unwrap();
        assert!((u.tokens_per_sec - 200.0).abs() < 1e-6);
        assert!(!u.provider_timed);
    }

    #[test]
    fn zero_provider_time_is_ignored() {
        let resp = json!({
            "usage": {"prompt_tokens": 10, "completion_tokens": 100},
            "time_info": {"completion_time": 0.0},
        });
        let u = RequestUsage::from_response(&resp, Duration::from_secs(1), RequestKind::Question)
            .unwrap();
        assert!((u.tokens_per_sec - 100.0).abs() < 1e-6);
        assert!(!u.provider_timed);
    }

    #[test]
    fn no_usage_block_means_no_sample() {
        let resp = json!({"choices": [{"message": {"content": "{}"}}]});
        assert!(RequestUsage::from_response(
            &resp,
            Duration::from_millis(100),
            RequestKind::Question
        )
        .is_none());
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
        stats.record_failure(RequestKind::Prefetch);
        assert_eq!(stats.kind(RequestKind::Question).requests, 1);
        assert_eq!(stats.kind(RequestKind::Monster).tokens, 150);
        assert_eq!(stats.kind(RequestKind::Prefetch).failures, 1);
        assert_eq!(stats.kind(RequestKind::Lesson), KindTotals::default());
        assert_eq!(stats.requests, 2);
        assert_eq!(stats.failures, 1);
    }
}
