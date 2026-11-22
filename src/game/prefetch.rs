//! Look-ahead for questions (FR32): the game knows where every flask and
//! monster is long before the player gets there, so it asks for their
//! questions early and keeps them ready. This module is the runtime half:
//! the receivers in flight, the per-minute budget, and turning what's coming
//! up the road into what to ask for. The pure policy (`plan_prefetch`) and the
//! buffer itself live in `llm::buffer`.
//!
//! Prefetching only fills the buffer. It never starts a question: that's
//! `Game::request_task`, still the only door (invariant 6).

use crate::content::lessons::normalize_topic;
use crate::content::prompts::topics_for_level;
use crate::content::studio::LessonJob;
use crate::game::world::{GoalKind, Upcoming};
use crate::llm::activity::{GenActivity, JobState};
use crate::llm::buffer::{plan_prefetch, PrefetchCaps, QuestionBuffer, Source, LEVELS};
use crate::llm::cerebras_client::CerebrasClient;
use crate::llm::usage::RequestKind;
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
    /// Its job on the creation bar, if one was opened (FR33).
    pub job: Option<u32>,
}

struct Flight {
    difficulty: u32,
    /// The topic asked for, for a scheme's flasks (FR23 C).
    topic: Option<String>,
    /// Its circle on the creation bar. A request adopted from a live one has
    /// none: that one's circle was ended or taken off already.
    job: Option<u32>,
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
    /// live as it always did. Each look-ahead's circle on the creation bar
    /// closes as it lands (FR33); a failed one is taken off quietly, since
    /// nothing was shown in its place.
    pub fn poll(&mut self, activity: &mut GenActivity) {
        let buffer = &mut self.buffer;
        self.flying.retain(|f| {
            let landed = match f.rx.try_recv() {
                Ok(Ok(task)) => {
                    if task.answer.is_finite() {
                        buffer.push(f.difficulty, task);
                    }
                    true
                }
                Ok(Err(_)) | Err(TryRecvError::Disconnected) => false,
                Err(TryRecvError::Empty) => return true,
            };
            if let Some(job) = f.job {
                if landed {
                    activity.finish(job, JobState::Ready);
                } else {
                    activity.cancel(job);
                }
            }
            false
        });
    }

    /// Keeps the answer to a request that's no longer needed (a late reply
    /// after the soft deadline, a battle that ended first): it goes into the
    /// buffer when it lands instead of being thrown away.
    pub fn adopt(&mut self, difficulty: u32, rx: Receiver<anyhow::Result<MathTask>>) {
        self.flying.push(Flight {
            difficulty,
            topic: None,
            job: None,
            rx,
        });
    }

    /// Questions on `topic` at `difficulty` that are ready or on their way.
    pub fn themed_count(&self, difficulty: u32, topic: &str) -> usize {
        let flying = self
            .flying
            .iter()
            .filter(|f| {
                f.difficulty == difficulty
                    && f.topic
                        .as_deref()
                        .is_some_and(|t| crate::llm::buffer::same_topic(t, topic))
            })
            .count();
        self.buffer.count_topic(difficulty, topic) + flying
    }

    /// Starts whatever look-ahead requests the plan calls for. The caller has
    /// already ruled out the times it must not: while paused, while a live
    /// question is loading. `themed` is a scheme's flasks (FR23 C):
    /// `(difficulty, topic, how many)` questions on that topic, asked for
    /// first, under the same caps.
    pub fn tick(
        &mut self,
        client: &CerebrasClient,
        wants: &[(u32, usize)],
        themed: Option<(u32, &str, usize)>,
        now: Instant,
        activity: &mut GenActivity,
    ) {
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
        if let Some((difficulty, topic, want)) = themed {
            let max = PrefetchCaps::default().max_in_flight;
            while self.themed_count(difficulty, topic) < want
                && self.flying.len() < max
                && self.sent.len() < self.cfg.rpm
            {
                self.sent.push_back(now);
                self.flying.push(Flight {
                    difficulty,
                    topic: Some(topic.to_string()),
                    job: Some(activity.start(RequestKind::Prefetch, &format!("scheme: {topic}"))),
                    rx: client.prefetch_task_async(difficulty, Some(topic.to_string())),
                });
            }
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
                topic: None,
                job: Some(
                    activity.start(RequestKind::Prefetch, &format!("ahead, level {difficulty}")),
                ),
                rx: client.prefetch_task_async(difficulty, None),
            });
        }
    }
}

/// Ready questions at the player's level below which the studio writes a lesson pack.
pub const LESSON_LOW: usize = 3;
/// Problems asked for in a pack, and in a "similar practice" pack.
pub const LESSON_N: usize = 8;
pub const LIKE_N: usize = 4;

/// Which lesson pack, if any, the studio should write next (FR22). Pure.
///
/// A problem the player missed comes first: a small pack of similar ones at
/// its level. Otherwise, when fewer than `LESSON_LOW` questions are ready at
/// the player's `level`, a full pack on the topic they're weakest at, or, if
/// nothing is weak yet, the next topic in the level's rotation.
pub fn plan_lesson(
    level: u32,
    ready_at_level: usize,
    weakest: Option<&str>,
    missed: Option<&MathTask>,
    rotation: usize,
) -> Option<LessonJob> {
    if let Some(m) = missed {
        return Some(LessonJob {
            topic: normalize_topic(&m.topic),
            level: m.difficulty.clamp(1, LEVELS as u32),
            n: LIKE_N,
            like: Some(m.problem.clone()),
        });
    }
    if ready_at_level >= LESSON_LOW {
        return None;
    }
    let topics = topics_for_level(level);
    let topic = weakest
        .map(str::to_string)
        .unwrap_or_else(|| topics[rotation % topics.len()].to_string());
    Some(LessonJob {
        topic,
        level: level.clamp(1, LEVELS as u32),
        n: LESSON_N,
        like: None,
    })
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

    fn missed(topic: &str, problem: &str, difficulty: u32) -> MathTask {
        MathTask {
            topic: topic.into(),
            problem: problem.into(),
            difficulty,
            ..task()
        }
    }

    #[test]
    fn a_missed_problem_comes_back_as_similar_practice_first() {
        let m = missed("Fractions", "What is 1/2 of 8?", 3);
        let job = plan_lesson(2, 99, Some("division"), Some(&m), 0).unwrap();
        assert_eq!(
            job,
            LessonJob {
                topic: "fractions".into(),
                level: 3,
                n: LIKE_N,
                like: Some("What is 1/2 of 8?".into())
            }
        );
    }

    #[test]
    fn a_low_buffer_asks_for_the_weakest_topic_else_the_rotation() {
        let job = plan_lesson(3, 1, Some("division"), None, 7).unwrap();
        assert_eq!(
            (job.topic.as_str(), job.level, job.n),
            ("division", 3, LESSON_N)
        );
        assert_eq!(job.like, None);
        let a = plan_lesson(3, 0, None, None, 0).unwrap();
        let b = plan_lesson(3, 0, None, None, 1).unwrap();
        assert_ne!(a.topic, b.topic, "the rotation moves on");
        assert!(topics_for_level(3).contains(&a.topic.as_str()));
        assert_eq!(plan_lesson(9, 0, None, None, 0).unwrap().level, 5);
    }

    #[test]
    fn a_stocked_buffer_needs_no_pack() {
        assert_eq!(plan_lesson(2, LESSON_LOW, Some("division"), None, 0), None);
        assert!(plan_lesson(2, LESSON_LOW - 1, None, None, 0).is_some());
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
        p.poll(&mut GenActivity::new());
        // Still waiting on the third.
        assert_eq!(p.in_flight(), 1);
        assert_eq!(p.buffer.counts(), [0, 1, 0, 0, 0]);
        drop(tx3);
        p.poll(&mut GenActivity::new());
        assert_eq!(p.in_flight(), 0);
    }

    #[test]
    fn a_look_ahead_has_a_circle_that_closes_when_it_lands_or_goes_quietly_when_it_fails() {
        let mut p = Prefetch::new(PrefetchConfig::parse(None, None));
        let mut a = GenActivity::new();
        let (tx1, rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();
        let j1 = a.start(RequestKind::Prefetch, "ahead, level 2");
        let j2 = a.start(RequestKind::Prefetch, "ahead, level 3");
        p.flying.push(Flight {
            difficulty: 2,
            topic: None,
            job: Some(j1),
            rx: rx1,
        });
        p.flying.push(Flight {
            difficulty: 3,
            topic: None,
            job: Some(j2),
            rx: rx2,
        });
        p.poll(&mut a);
        assert_eq!(a.running(), 2, "nothing has landed yet");
        tx1.send(Ok(task())).unwrap();
        tx2.send(Err(anyhow::anyhow!("boom"))).unwrap();
        p.poll(&mut a);
        assert_eq!(a.running(), 0);
        assert_eq!(a.jobs().len(), 1, "the failed one is taken off, not shown");
        assert_eq!(a.jobs()[0].state, JobState::Ready);
    }

    #[test]
    fn sending_a_look_ahead_opens_a_circle_for_it() {
        let mut p = Prefetch::new(PrefetchConfig::parse(None, None));
        let client = CerebrasClient::new(&crate::config::Config {
            api_key: Some("k".into()),
            model: "m".into(),
            base_url: "http://127.0.0.1:9".into(),
        })
        .with_fake_model(Duration::from_millis(5));
        let mut a = GenActivity::new();
        p.tick(&client, &[(2, 2)], None, Instant::now(), &mut a);
        assert_eq!(p.in_flight(), a.running());
        assert!(a.running() > 0);
        assert!(a.jobs().iter().all(|j| j.kind == RequestKind::Prefetch));
    }

    #[test]
    fn a_scheme_asks_for_its_topic_first_and_stops_when_stocked() {
        let mut p = Prefetch::new(PrefetchConfig::parse(None, None));
        let client = CerebrasClient::new(&crate::config::Config {
            api_key: Some("k".into()),
            model: "m".into(),
            base_url: "http://127.0.0.1:9".into(),
        })
        .with_fake_model(Duration::from_millis(1));
        let mut a = GenActivity::new();
        p.tick(
            &client,
            &[(2, 2)],
            Some((2, "fractions", 3)),
            Instant::now(),
            &mut a,
        );
        // Both slots in flight go to the scheme's topic, ahead of the plain plan.
        assert_eq!(p.in_flight(), 2);
        assert_eq!(p.themed_count(2, "Fractions"), 2);
        // Once they land they're on the topic, and a stocked scheme asks for no more.
        std::thread::sleep(Duration::from_millis(60));
        p.poll(&mut a);
        assert_eq!(p.buffer.count_topic(2, "fractions"), 2);
        p.buffer.push(
            2,
            MathTask {
                topic: "fractions".into(),
                ..task()
            },
        );
        let before = p.sent.len();
        p.tick(
            &client,
            &[],
            Some((2, "fractions", 3)),
            Instant::now(),
            &mut a,
        );
        assert_eq!(p.sent.len(), before, "three are ready: nothing more to ask");
    }

    #[test]
    fn a_disabled_prefetcher_asks_for_nothing() {
        let mut p = Prefetch::new(PrefetchConfig::parse(Some("off"), None));
        let client = CerebrasClient::new(&crate::config::Config {
            api_key: Some("k".into()),
            model: "m".into(),
            base_url: "http://127.0.0.1:9".into(),
        });
        let mut a = GenActivity::new();
        p.tick(&client, &[(1, 3)], None, Instant::now(), &mut a);
        assert_eq!(p.in_flight(), 0);
        assert!(a.is_empty());
    }
}
