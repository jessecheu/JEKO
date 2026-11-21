//! Look-ahead for questions (FR32): the game knows where every flask and
//! monster is long before the player gets there, so it asks for their
//! questions early and keeps them ready. This module is the runtime half:
//! the receivers in flight, the per-minute budget, and turning what's coming
//! up the road into what to ask for. The pure policy (`plan_prefetch`) and the
//! buffer itself live in `llm::buffer`.
//!
//! Prefetching only fills the buffer. It never starts a question: that's
//! `Game::request_task`, still the only door (invariant 6).

use crate::game::world::{GoalKind, Upcoming};
use crate::llm::buffer::{plan_prefetch, PrefetchCaps, QuestionBuffer, Source, LEVELS};
use crate::llm::cerebras_client::CerebrasClient;
use crate::llm::MathTask;
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// How far up the road the game looks for things that will need a question,
/// in screen heights.
pub const LOOKAHEAD_SCREENS: f32 = 1.5;
/// How long a battle question may take before a bank question is shown
/// instead and the late answer is kept for next time.
pub const SOFT_DEADLINE_BATTLE_S: f32 = 1.5;
/// The same at a flask, where there's no animation covering the wait.
pub const SOFT_DEADLINE_FLASK_S: f32 = 2.5;
/// Prefetches per minute unless `JEKO_PREFETCH_RPM` says otherwise.
pub const DEFAULT_RPM: usize = 20;

/// Settings read once at startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrefetchConfig {
    /// `JEKO_PREFETCH=off` turns the look-ahead off.
    pub enabled: bool,
    /// `JEKO_PREFETCH_RPM`: look-ahead requests per minute.
    pub rpm: usize,
}

impl PrefetchConfig {
    pub fn from_env() -> Self {
        Self::parse(
            std::env::var("JEKO_PREFETCH").ok().as_deref(),
            std::env::var("JEKO_PREFETCH_RPM").ok().as_deref(),
        )
    }

    fn parse(switch: Option<&str>, rpm: Option<&str>) -> Self {
        Self {
            enabled: !matches!(
                switch.map(|s| s.trim().to_ascii_lowercase()).as_deref(),
                Some("off" | "0" | "false" | "no")
            ),
            rpm: rpm
                .and_then(|r| r.trim().parse().ok())
                .unwrap_or(DEFAULT_RPM),
        }
    }
}

/// A question that has been asked for and not yet shown, with what's needed
/// to measure how long the player waited for it.
#[derive(Clone, Copy, Debug)]
pub struct Ask {
    pub difficulty: u32,
    pub source: Source,
    /// Seconds the player has watched the spinner (game time).
    pub waited: f32,
    /// After this long a bank question is shown instead.
    pub deadline: f32,
    /// False when there's no model to wait for (offline): nothing to measure.
    pub counts: bool,
}

struct Flight {
    difficulty: u32,
    rx: Receiver<anyhow::Result<MathTask>>,
}

pub struct Prefetch {
    cfg: PrefetchConfig,
    pub buffer: QuestionBuffer,
    flying: Vec<Flight>,
    /// When each look-ahead request of the last minute was sent.
    sent: VecDeque<Instant>,
}

impl Prefetch {
    pub fn new(cfg: PrefetchConfig) -> Self {
        Self {
            cfg,
            buffer: QuestionBuffer::default(),
            flying: Vec::new(),
            sent: VecDeque::new(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    pub fn in_flight(&self) -> usize {
        self.flying.len()
    }

    fn in_flight_counts(&self) -> [usize; LEVELS] {
        let mut counts = [0; LEVELS];
        for f in &self.flying {
            counts[(f.difficulty.clamp(1, LEVELS as u32) - 1) as usize] += 1;
        }
        counts
    }

    /// Moves answers that have landed into the buffer. Runs every frame,
    /// paused or not, so one that lands mid-pause is kept, not lost. A failed
    /// call is just dropped: the buffer stays short and `request_task` asks
    /// live as it always did.
    pub fn poll(&mut self) {
        let buffer = &mut self.buffer;
        self.flying.retain(|f| match f.rx.try_recv() {
            Ok(Ok(task)) => {
                if task.answer.is_finite() {
                    buffer.push(f.difficulty, task);
                }
                false
            }
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => false,
            Err(TryRecvError::Empty) => true,
        });
    }

    /// Keeps the answer to a request that's no longer needed (a late reply
    /// after the soft deadline, a battle that ended first): it goes into the
    /// buffer when it lands instead of being thrown away.
    pub fn adopt(&mut self, difficulty: u32, rx: Receiver<anyhow::Result<MathTask>>) {
        self.flying.push(Flight { difficulty, rx });
    }

    /// Starts whatever look-ahead requests the plan calls for. The caller has
    /// already ruled out the times it must not: while paused, while a live
    /// question is loading.
    pub fn tick(&mut self, client: &CerebrasClient, wants: &[(u32, usize)], now: Instant) {
        if !self.cfg.enabled || !client.has_api_key() {
            return;
        }
        while self
            .sent
            .front()
            .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(60))
        {
            self.sent.pop_front();
        }
        let caps = PrefetchCaps {
            budget: self.cfg.rpm.saturating_sub(self.sent.len()),
            ..PrefetchCaps::default()
        };
        let plan = plan_prefetch(wants, self.buffer.counts(), self.in_flight_counts(), caps);
        for difficulty in plan {
            self.sent.push_back(now);
            self.flying.push(Flight {
                difficulty,
                rx: client.prefetch_task_async(difficulty),
            });
        }
    }
}

/// What the road ahead will need, nearest first, as `(difficulty, how many)`:
/// a flask wants one question at the player's level, and a monster wants one
/// per hit at `level + its bias`, plus a spare for a miss.
pub fn demand(upcoming: &[Upcoming], level: u32) -> Vec<(u32, usize)> {
    upcoming
        .iter()
        .map(|u| {
            let difficulty = (level + u.difficulty_bias).min(LEVELS as u32);
            let count = match u.kind {
                GoalKind::Flask => 1,
                _ => u.hits as usize + 1,
            };
            (difficulty, count)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::prelude::vec2;
    use std::sync::mpsc;

    fn up(kind: GoalKind, hits: u32, bias: u32) -> Upcoming {
        Upcoming {
            kind,
            y: -100.0,
            at: vec2(0.0, -100.0),
            hits,
            difficulty_bias: bias,
        }
    }

    fn task() -> MathTask {
        MathTask {
            topic: "t".into(),
            problem: "p".into(),
            answer: 1.0,
            hint: String::new(),
            difficulty: 1,
            explanation: Vec::new(),
            expression: None,
        }
    }

    #[test]
    fn a_flask_wants_one_and_a_monster_wants_its_hits_and_a_spare() {
        let ups = [
            up(GoalKind::Flask, 1, 0),
            up(GoalKind::Patch, 2, 1),
            up(GoalKind::Titan, 5, 3),
        ];
        // Level 2: the flask at 2, the patch at 3, the boss clamped to 5.
        assert_eq!(demand(&ups, 2), vec![(2, 1), (3, 3), (5, 6)]);
    }

    #[test]
    fn env_switches_parse() {
        let d = PrefetchConfig::parse(None, None);
        assert!(d.enabled);
        assert_eq!(d.rpm, DEFAULT_RPM);
        assert!(!PrefetchConfig::parse(Some("off"), None).enabled);
        assert!(!PrefetchConfig::parse(Some(" OFF "), None).enabled);
        assert!(PrefetchConfig::parse(Some("on"), None).enabled);
        assert_eq!(PrefetchConfig::parse(None, Some("7")).rpm, 7);
        assert_eq!(PrefetchConfig::parse(None, Some("lots")).rpm, DEFAULT_RPM);
    }

    #[test]
    fn answers_that_land_go_to_the_buffer_and_failures_vanish() {
        let mut p = Prefetch::new(PrefetchConfig::parse(None, None));
        let (tx1, rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();
        let (tx3, rx3) = mpsc::channel::<anyhow::Result<MathTask>>();
        p.adopt(2, rx1);
        p.adopt(3, rx2);
        p.adopt(4, rx3);
        assert_eq!(p.in_flight(), 3);
        tx1.send(Ok(task())).unwrap();
        tx2.send(Err(anyhow::anyhow!("boom"))).unwrap();
        p.poll();
        // Still waiting on the third.
        assert_eq!(p.in_flight(), 1);
        assert_eq!(p.buffer.counts(), [0, 1, 0, 0, 0]);
        drop(tx3);
        p.poll();
        assert_eq!(p.in_flight(), 0);
    }

    #[test]
    fn a_disabled_prefetcher_asks_for_nothing() {
        let mut p = Prefetch::new(PrefetchConfig::parse(Some("off"), None));
        let client = CerebrasClient::new(&crate::config::Config {
            api_key: Some("k".into()),
            model: "m".into(),
            base_url: "http://127.0.0.1:9".into(),
        });
        p.tick(&client, &[(1, 3)], Instant::now());
        assert_eq!(p.in_flight(), 0);
    }
}
