//! The doctor's live voice (FR23 B): the one model call for a line that may be
//! out at a time, its per-minute cap, and where each reply goes. The pure half
//! (the prompt and the checks on a reply) is `llm::villain_gen`; the doctor
//! (`game::villain`) only ever sees a line that passed them.
//!
//! The villain takes the third lane, after live questions and question
//! look-ahead (FR32): `Game::tick_voice` starts nothing while a question is
//! loading, a look-ahead is in flight, the game is paused or a battle is on,
//! and the studio holds its own jobs while a line is out. A line that doesn't
//! come (no key, `JEKO_VILLAIN=off`, the cap, a failed or turned-down reply)
//! means the doctor says one from `assets/villain_lines.json`: the game never
//! waits for this and plays the same without it.

use super::villain::{Beat, Villain};
use crate::llm::activity::{GenActivity, JobState};
use crate::llm::cerebras_client::CerebrasClient;
use crate::llm::villain_gen::{VillainAsk, VillainReply};
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// Live villain calls per minute unless `JEKO_VILLAIN_RPM` says otherwise.
pub const DEFAULT_RPM: usize = 6;

/// Settings read once at startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceConfig {
    /// `JEKO_VILLAIN=off` keeps the doctor on the line bank.
    pub enabled: bool,
    /// `JEKO_VILLAIN_RPM`: live villain calls per minute.
    pub rpm: usize,
}

impl VoiceConfig {
    pub fn from_env() -> Self {
        Self::parse(
            std::env::var("JEKO_VILLAIN").ok().as_deref(),
            std::env::var("JEKO_VILLAIN_RPM").ok().as_deref(),
        )
    }

    pub fn parse(switch: Option<&str>, rpm: Option<&str>) -> Self {
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

/// What a call in flight is for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Purpose {
    /// The line for the waiting cue with this number.
    Cue(u32),
    /// Both reactions to the flask at `spot`. `answer` is set if the player
    /// answers before they land: which one is wanted, and for which cue.
    Pair {
        spot: usize,
        answer: Option<(bool, u32)>,
    },
}

struct Flight {
    purpose: Purpose,
    /// Its circle on the creation bar (FR33).
    job: u32,
    rx: Receiver<anyhow::Result<VillainReply>>,
}

/// How a flask's reaction stands when the player answers.
#[derive(Clone, Debug, PartialEq)]
pub enum Reaction {
    /// Here already: the doctor reacts on this frame.
    Ready(Beat),
    /// Still being written: route it to the cue when it lands.
    Coming,
    /// Never asked for (or capped): the cue asks, or uses the bank.
    Nothing,
}

/// Counts for the debug readout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VoiceStats {
    /// Calls sent.
    pub asked: u32,
    /// Replies that passed the checks.
    pub landed: u32,
    /// Calls that failed or were turned down.
    pub failed: u32,
    /// Lines that went to the bank because the minute's cap was spent.
    pub capped: u32,
    /// Lines that came back after their cue was gone.
    pub late: u32,
}

pub struct Voice {
    cfg: VoiceConfig,
    flight: Option<Flight>,
    /// When each call of the last minute was sent.
    sent: VecDeque<Instant>,
    /// Both reactions to the open flask, once they've landed: its spot, then
    /// right and wrong.
    pair: Option<(usize, Beat, Beat)>,
    /// The flask whose reactions have been asked for (or skipped), so they're
    /// asked for once.
    pair_asked: Option<usize>,
    pub stats: VoiceStats,
}

impl Voice {
    pub fn new(cfg: VoiceConfig) -> Self {
        Self {
            cfg,
            flight: None,
            sent: VecDeque::new(),
            pair: None,
            pair_asked: None,
            stats: VoiceStats::default(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    /// A call is out.
    pub fn in_flight(&self) -> bool {
        self.flight.is_some()
    }

    /// Calls this minute's cap still allows.
    pub fn budget(&mut self, now: Instant) -> usize {
        while self
            .sent
            .front()
            .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(60))
        {
            self.sent.pop_front();
        }
        self.cfg.rpm.saturating_sub(self.sent.len())
    }

    /// The flask at `spot` hasn't had its reactions asked for yet.
    pub fn wants_pair(&self, spot: usize) -> bool {
        self.pair_asked != Some(spot)
    }

    /// The reactions to `spot` won't be asked for (the cap is spent).
    pub fn skip_pair(&mut self, spot: usize) {
        self.pair_asked = Some(spot);
        self.stats.capped += 1;
    }

    fn send(
        &mut self,
        client: &CerebrasClient,
        purpose: Purpose,
        ask: VillainAsk,
        label: &str,
        activity: &mut GenActivity,
        now: Instant,
    ) {
        self.sent.push_back(now);
        self.stats.asked += 1;
        self.flight = Some(Flight {
            purpose,
            job: activity.start(crate::llm::usage::RequestKind::Line, label),
            rx: client.request_villain_async(ask, crate::content::vet::vet_text),
        });
    }

    /// Asks for both reactions to the flask at `spot`. The caller has checked
    /// the lane is free and the cap has room.
    pub fn ask_pair(
        &mut self,
        client: &CerebrasClient,
        spot: usize,
        ask: VillainAsk,
        activity: &mut GenActivity,
        now: Instant,
    ) {
        self.pair_asked = Some(spot);
        self.pair = None;
        let purpose = Purpose::Pair { spot, answer: None };
        self.send(client, purpose, ask, "doctor's reactions", activity, now);
    }

    /// Asks for the line for waiting cue `id`.
    pub fn ask_cue(
        &mut self,
        client: &CerebrasClient,
        id: u32,
        ask: VillainAsk,
        activity: &mut GenActivity,
        now: Instant,
    ) {
        let label = match &ask {
            VillainAsk::One(a) if !a.scheme_topics.is_empty() => "doctor's scheme",
            _ => "doctor's line",
        };
        self.send(client, Purpose::Cue(id), ask, label, activity, now);
    }

    /// The reaction to the flask at `spot` for a `correct` (or wrong) answer.
    /// A ready pair is used up either way: the other half is dropped.
    pub fn reaction(&mut self, spot: usize, correct: bool) -> Reaction {
        if let Some((s, right, wrong)) = self.pair.take() {
            if s == spot {
                return Reaction::Ready(if correct { right } else { wrong });
            }
        }
        match &self.flight {
            Some(Flight {
                purpose: Purpose::Pair { spot: s, .. },
                ..
            }) if *s == spot => Reaction::Coming,
            _ => Reaction::Nothing,
        }
    }

    /// The flask's reactions are still in flight and the player has answered:
    /// the `correct` (or wrong) half goes to cue `id` when they land.
    pub fn route_answer(&mut self, correct: bool, id: u32) {
        if let Some(Flight {
            purpose: Purpose::Pair { answer, .. },
            ..
        }) = &mut self.flight
        {
            *answer = Some((correct, id));
        }
    }

    /// Hands what has landed to the doctor. Runs every frame, paused or not,
    /// so a reply isn't lost. Each call's circle on the creation bar closes as
    /// it lands, or shows `bank` when the doctor will say a bank line instead.
    pub fn poll(&mut self, villain: &mut Villain, activity: &mut GenActivity) {
        let Some(f) = &self.flight else {
            return;
        };
        let result = match f.rx.try_recv() {
            Err(TryRecvError::Empty) => return,
            Ok(Ok(reply)) => Some(reply),
            Ok(Err(e)) => {
                // One line on stderr, so a model that keeps answering badly can
                // be seen; the doctor says a bank line either way.
                eprintln!("villain: no live line ({e:#}); using the line bank");
                None
            }
            Err(TryRecvError::Disconnected) => None,
        };
        let Some(f) = self.flight.take() else {
            return;
        };
        match result {
            Some(reply) => {
                self.stats.landed += 1;
                activity.finish(f.job, JobState::Ready);
                let used = match (f.purpose, reply) {
                    (Purpose::Cue(id), VillainReply::One(beat)) => villain.offer(id, beat),
                    (Purpose::Pair { spot, answer }, VillainReply::Pair { right, wrong }) => {
                        match answer {
                            Some((correct, id)) => {
                                villain.offer(id, if correct { right } else { wrong })
                            }
                            None => {
                                self.pair = Some((spot, right, wrong));
                                true
                            }
                        }
                    }
                    _ => false,
                };
                if !used {
                    self.stats.late += 1;
                }
            }
            None => {
                self.stats.failed += 1;
                activity.finish(f.job, JobState::Fallback);
                match f.purpose {
                    Purpose::Cue(id)
                    | Purpose::Pair {
                        answer: Some((_, id)),
                        ..
                    } => villain.give_up(id),
                    Purpose::Pair { .. } => {}
                }
            }
        }
    }

    /// A new run: a ready pair belongs to a flask of the old world. A call
    /// still out lands harmlessly (the cue numbers don't repeat).
    pub fn reset_run(&mut self) {
        self.pair = None;
        self.pair_asked = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::villain::{Mood, VillainAction, VillainCue};
    use crate::llm::villain_gen::RunContext;

    fn beat(line: &str) -> Beat {
        Beat {
            line: line.into(),
            mood: Mood::Smug,
            action: VillainAction::Gloat,
            scheme: None,
        }
    }

    fn client() -> CerebrasClient {
        CerebrasClient::new(&crate::config::Config {
            api_key: Some("k".into()),
            model: "m".into(),
            base_url: "http://127.0.0.1:9".into(),
        })
        .with_fake_model(Duration::from_millis(1))
    }

    fn wait_for(voice: &mut Voice, v: &mut Villain, a: &mut GenActivity) {
        for _ in 0..200 {
            voice.poll(v, a);
            if !voice.in_flight() {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("the stand-in model never answered");
    }

    #[test]
    fn env_switches_parse() {
        assert_eq!(
            VoiceConfig::parse(None, None),
            VoiceConfig {
                enabled: true,
                rpm: DEFAULT_RPM
            }
        );
        assert!(!VoiceConfig::parse(Some(" OFF "), None).enabled);
        assert!(!VoiceConfig::parse(Some("0"), None).enabled);
        assert_eq!(VoiceConfig::parse(None, Some("12")).rpm, 12);
        assert_eq!(VoiceConfig::parse(None, Some("lots")).rpm, DEFAULT_RPM);
    }

    #[test]
    fn the_minute_cap_is_kept() {
        let mut voice = Voice::new(VoiceConfig::parse(None, Some("2")));
        let now = Instant::now();
        assert_eq!(voice.budget(now), 2);
        voice.sent.push_back(now);
        voice.sent.push_back(now);
        assert_eq!(voice.budget(now), 0);
        assert_eq!(
            voice.budget(now + Duration::from_secs(61)),
            2,
            "a minute later"
        );
    }

    #[test]
    fn a_ready_pair_gives_the_matching_half_once() {
        let mut voice = Voice::new(VoiceConfig::parse(None, None));
        voice.pair = Some((4, beat("right"), beat("wrong")));
        assert_eq!(voice.reaction(3, true), Reaction::Nothing, "another flask");
        voice.pair = Some((4, beat("right"), beat("wrong")));
        assert_eq!(voice.reaction(4, false), Reaction::Ready(beat("wrong")));
        assert_eq!(voice.reaction(4, true), Reaction::Nothing, "used up");
    }

    #[test]
    fn a_live_line_reaches_the_cue_it_was_asked_for() {
        let mut v = Villain::out_for_test();
        let mut a = GenActivity::new();
        let mut voice = Voice::new(VoiceConfig::parse(None, None));
        let id = v.cue(VillainCue::Idle).unwrap();
        let ask = VillainAsk::One(v.beat_ask(VillainCue::Idle, RunContext::default(), &[]));
        v.mark_asked(id);
        voice.ask_cue(&client(), id, ask, &mut a, Instant::now());
        assert!(voice.in_flight() && v.wants_line().is_none());
        wait_for(&mut voice, &mut v, &mut a);
        assert_eq!(voice.stats.landed, 1);
        assert!(v.pending_line().is_some_and(|l| l.contains("stand-in")));
        assert_eq!(a.jobs()[0].state, JobState::Ready);
    }

    #[test]
    fn a_pair_in_flight_goes_to_the_answer_when_it_lands() {
        let mut v = Villain::out_for_test();
        let mut a = GenActivity::new();
        let mut voice = Voice::new(VoiceConfig::parse(None, None));
        let ctx = RunContext::default();
        let ask = VillainAsk::Pair {
            right: v.reaction_ask(true, ctx.clone()),
            wrong: v.reaction_ask(false, ctx),
        };
        voice.ask_pair(&client(), 7, ask, &mut a, Instant::now());
        assert!(!voice.wants_pair(7));
        assert_eq!(voice.reaction(7, false), Reaction::Coming);
        let id = v.cue(VillainCue::FlaskWrong).unwrap();
        v.mark_asked(id);
        voice.route_answer(false, id);
        wait_for(&mut voice, &mut v, &mut a);
        let line = v.pending_line().expect("the wrong half");
        assert!(line.contains("flask wrong"), "{line}");
    }

    #[test]
    fn a_late_reply_for_a_replaced_cue_is_dropped() {
        let mut v = Villain::out_for_test();
        let mut a = GenActivity::new();
        let mut voice = Voice::new(VoiceConfig::parse(None, None));
        let id = v.cue(VillainCue::Idle).unwrap();
        let ask = VillainAsk::One(v.beat_ask(VillainCue::Idle, RunContext::default(), &[]));
        voice.ask_cue(&client(), id, ask, &mut a, Instant::now());
        // Something bigger comes along before the line is back.
        v.cue(VillainCue::LevelUp { level: 3 }).unwrap();
        wait_for(&mut voice, &mut v, &mut a);
        assert_eq!(voice.stats.late, 1);
        assert!(
            v.pending_line().is_none(),
            "the level-up isn't given the idle line"
        );
    }

    #[test]
    fn a_failed_call_sends_the_cue_to_the_bank() {
        let mut v = Villain::out_for_test();
        let mut a = GenActivity::new();
        let mut voice = Voice::new(VoiceConfig::parse(None, None));
        // A real client with no server to answer: the call fails.
        let dead = CerebrasClient::new(&crate::config::Config {
            api_key: Some("k".into()),
            model: "m".into(),
            base_url: "http://127.0.0.1:9".into(),
        });
        let id = v.cue(VillainCue::Idle).unwrap();
        v.mark_asked(id);
        let ask = VillainAsk::One(v.beat_ask(VillainCue::Idle, RunContext::default(), &[]));
        voice.ask_cue(&dead, id, ask, &mut a, Instant::now());
        for _ in 0..400 {
            voice.poll(&mut v, &mut a);
            if !voice.in_flight() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(voice.stats.failed, 1);
        assert!(v.pending_failed());
        assert_eq!(a.jobs()[0].state, JobState::Fallback);
    }
}
