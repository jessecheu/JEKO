//! Questions that are ready before anyone asks for them (FR32), and the
//! readout of how long the player still waited. Plain Rust with no game types:
//! `game::prefetch` decides *what* to ask for and feeds this.

use super::MathTask;
use std::collections::VecDeque;

/// Difficulties run 1 to 5.
pub const LEVELS: usize = 5;

/// A few ready `MathTask`s per difficulty. Filled by prefetches (FR32) and,
/// in bulk, by lesson packs (FR22); drained by `Game::request_task`.
#[derive(Default)]
pub struct QuestionBuffer {
    slots: [VecDeque<MathTask>; LEVELS],
}

fn slot(difficulty: u32) -> usize {
    (difficulty.clamp(1, LEVELS as u32) - 1) as usize
}

impl QuestionBuffer {
    /// Files `task` under the difficulty it was asked for (not the one the
    /// model says it wrote: a model can be off by one).
    pub fn push(&mut self, difficulty: u32, task: MathTask) {
        self.slots[slot(difficulty)].push_back(task);
    }

    /// Ready questions per difficulty, index 0 = difficulty 1.
    pub fn counts(&self) -> [usize; LEVELS] {
        std::array::from_fn(|i| self.slots[i].len())
    }

    pub fn len(&self) -> usize {
        self.slots.iter().map(VecDeque::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Like `push`, but not if a question with the same text is already
    /// waiting, so reloading a lesson pack file doesn't double its questions.
    /// Returns whether it was added.
    pub fn push_unique(&mut self, difficulty: u32, task: MathTask) -> bool {
        if self
            .slots
            .iter()
            .flatten()
            .any(|t| t.problem == task.problem)
        {
            return false;
        }
        self.push(difficulty, task);
        true
    }

    /// Ready questions on `topic` at exactly `difficulty`.
    pub fn count_topic(&self, difficulty: u32, topic: &str) -> usize {
        self.slots[slot(difficulty)]
            .iter()
            .filter(|t| same_topic(&t.topic, topic))
            .count()
    }

    /// A ready question for `difficulty`: the exact one if there is one, else
    /// one a level below (a question a level too easy is better than a wait,
    /// and a level-up can leave the buffer one behind). With a `topic` (a
    /// monster's theme, FR22) only a question on that topic will do, so a
    /// fraction-loving jellyfish never asks about eggs; no match means `None`
    /// and the caller asks live with the topic in the prompt.
    pub fn take(&mut self, difficulty: u32, topic: Option<&str>) -> Option<MathTask> {
        let d = slot(difficulty);
        let levels = [Some(d), d.checked_sub(1)];
        for i in levels.into_iter().flatten() {
            let q = &mut self.slots[i];
            let at = match topic {
                Some(t) => q.iter().position(|task| same_topic(&task.topic, t)),
                None => (!q.is_empty()).then_some(0),
            };
            if let Some(task) = at.and_then(|at| q.remove(at)) {
                return Some(task);
            }
        }
        None
    }
}

/// Topics match ignoring case and surrounding space.
pub fn same_topic(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// The limits on look-ahead requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrefetchCaps {
    /// Requests in flight at once.
    pub max_in_flight: usize,
    /// Ready plus in-flight questions per difficulty.
    pub per_difficulty: usize,
    /// How many more requests this minute's budget allows.
    pub budget: usize,
}

impl Default for PrefetchCaps {
    fn default() -> Self {
        Self {
            max_in_flight: 2,
            per_difficulty: 3,
            budget: usize::MAX,
        }
    }
}

/// What to ask for next. `wants` is what the road ahead will need, nearest
/// first, as `(difficulty, how many)`. `ready` and `in_flight` are per
/// difficulty. Returns the difficulties to request now, nearest need first.
///
/// Pure, so the policy is testable without a network. It never plans more
/// than the caps allow, and a want that's already covered by a ready or
/// in-flight question costs nothing.
pub fn plan_prefetch(
    wants: &[(u32, usize)],
    ready: [usize; LEVELS],
    in_flight: [usize; LEVELS],
    caps: PrefetchCaps,
) -> Vec<u32> {
    let flying: usize = in_flight.iter().sum();
    let room = caps.max_in_flight.saturating_sub(flying).min(caps.budget);
    let mut have: [usize; LEVELS] = std::array::from_fn(|i| ready[i] + in_flight[i]);
    let mut out = Vec::new();
    'wants: for &(difficulty, count) in wants {
        let i = slot(difficulty);
        for _ in 0..count {
            if out.len() >= room {
                break 'wants;
            }
            if have[i] > 0 {
                // Covered by something ready or on its way.
                have[i] -= 1;
                continue;
            }
            let asked = out.iter().filter(|d| slot(**d) == i).count();
            if ready[i] + in_flight[i] + asked >= caps.per_difficulty {
                break;
            }
            out.push(i as u32 + 1);
        }
    }
    out
}

/// Where a shown question came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Taken from the buffer: no wait.
    Buffer,
    /// Asked live, and back before it was needed (during a battle's own
    /// animations): no wait.
    Ready,
    /// Asked live, and the player waited for it.
    Live,
    /// The live call failed; the fallback bank stepped in.
    Fallback,
    /// The live call missed the soft deadline; the fallback bank stepped in
    /// and the late answer was kept for next time.
    Deadline,
}

impl Source {
    fn index(self) -> usize {
        self as usize
    }
}

/// How long the player waited on each question shown this session.
#[derive(Clone, Debug, Default)]
pub struct WaitStats {
    shown: u32,
    by_source: [u32; 5],
    /// Seconds.
    longest: f32,
    total: f32,
}

impl WaitStats {
    /// Records a question that was shown after the player waited `waited_s`.
    pub fn record(&mut self, source: Source, waited_s: f32) {
        self.shown += 1;
        self.by_source[source.index()] += 1;
        self.longest = self.longest.max(waited_s);
        self.total += waited_s;
    }

    pub fn shown(&self) -> u32 {
        self.shown
    }

    pub fn count(&self, source: Source) -> u32 {
        self.by_source[source.index()]
    }

    /// Questions that were there when needed.
    pub fn ready_on_arrival(&self) -> u32 {
        self.count(Source::Buffer) + self.count(Source::Ready)
    }

    pub fn longest_s(&self) -> f32 {
        self.longest
    }

    pub fn mean_s(&self) -> f32 {
        if self.shown == 0 {
            0.0
        } else {
            self.total / self.shown as f32
        }
    }

    /// "ready on arrival 18/20 · longest wait 0.3 s", or `None` before the
    /// first question.
    pub fn readout(&self) -> Option<String> {
        (self.shown > 0).then(|| {
            format!(
                "ready on arrival {}/{} · longest wait {:.1} s",
                self.ready_on_arrival(),
                self.shown,
                self.longest
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(topic: &str, d: u32) -> MathTask {
        MathTask {
            topic: topic.into(),
            problem: "1+1".into(),
            answer: 2.0,
            hint: String::new(),
            difficulty: d,
            explanation: Vec::new(),
            expression: None,
        }
    }

    #[test]
    fn take_prefers_exact_then_one_below_then_nothing() {
        let mut b = QuestionBuffer::default();
        b.push(2, task("a", 2));
        assert!(b.take(3, None).is_some(), "one below is fine");
        assert!(b.take(3, None).is_none());
        b.push(2, task("a", 2));
        assert!(b.take(1, None).is_none(), "never a harder question");
        assert!(b.take(4, None).is_none(), "only one level below");
        assert!(b.take(2, None).is_some());
        assert!(b.is_empty());
    }

    #[test]
    fn a_topic_asks_for_that_topic_only() {
        let mut b = QuestionBuffer::default();
        b.push(1, task("addition", 1));
        b.push(1, task("Fractions", 1));
        assert_eq!(b.take(1, Some(" fractions ")).unwrap().topic, "Fractions");
        assert!(
            b.take(1, Some("fractions")).is_none(),
            "no other topic will do"
        );
        assert_eq!(b.take(1, None).unwrap().topic, "addition");
    }

    #[test]
    fn the_same_question_is_not_added_twice() {
        let mut b = QuestionBuffer::default();
        assert!(b.push_unique(1, task("a", 1)));
        assert!(!b.push_unique(1, task("a", 1)));
        assert!(!b.push_unique(3, task("a", 3)), "any level");
        assert_eq!(b.len(), 1);
    }

    #[test]
    fn difficulty_is_clamped() {
        let mut b = QuestionBuffer::default();
        b.push(0, task("a", 0));
        b.push(99, task("a", 99));
        assert_eq!(b.counts(), [1, 0, 0, 0, 1]);
    }

    fn caps(max_in_flight: usize, per_difficulty: usize, budget: usize) -> PrefetchCaps {
        PrefetchCaps {
            max_in_flight,
            per_difficulty,
            budget,
        }
    }

    #[test]
    fn plans_what_is_missing_nearest_first() {
        let wants = [(2, 1), (3, 2)];
        let plan = plan_prefetch(&wants, [0; 5], [0; 5], caps(9, 9, 99));
        assert_eq!(plan, vec![2, 3, 3]);
    }

    #[test]
    fn ready_and_in_flight_questions_cover_a_want() {
        let wants = [(2, 1), (3, 2)];
        let plan = plan_prefetch(&wants, [0, 1, 0, 0, 0], [0, 0, 1, 0, 0], caps(9, 9, 99));
        assert_eq!(plan, vec![3]);
    }

    #[test]
    fn the_in_flight_cap_counts_what_is_already_flying() {
        let wants = [(1, 1), (2, 1), (3, 1)];
        assert_eq!(
            plan_prefetch(&wants, [0; 5], [0; 5], caps(2, 9, 99)),
            vec![1, 2]
        );
        assert_eq!(
            plan_prefetch(&wants, [0; 5], [0, 0, 0, 0, 1], caps(2, 9, 99)),
            vec![1]
        );
        assert!(plan_prefetch(&wants, [0; 5], [1, 1, 0, 0, 0], caps(2, 9, 99)).is_empty());
    }

    #[test]
    fn the_minute_budget_is_a_cap() {
        let wants = [(1, 3)];
        assert_eq!(
            plan_prefetch(&wants, [0; 5], [0; 5], caps(9, 9, 1)).len(),
            1
        );
        assert!(plan_prefetch(&wants, [0; 5], [0; 5], caps(9, 9, 0)).is_empty());
    }

    #[test]
    fn at_most_three_ready_per_difficulty() {
        let wants = [(4, 5)];
        let plan = plan_prefetch(&wants, [0; 5], [0; 5], caps(9, 3, 99));
        assert_eq!(plan, vec![4, 4, 4]);
        let plan = plan_prefetch(&wants, [0, 0, 0, 2, 0], [0; 5], caps(9, 3, 99));
        assert_eq!(plan, vec![4]);
        assert!(plan_prefetch(&wants, [0, 0, 0, 3, 0], [0; 5], caps(9, 3, 99)).is_empty());
    }

    #[test]
    fn nothing_wanted_means_nothing_asked() {
        assert!(plan_prefetch(&[], [0; 5], [0; 5], PrefetchCaps::default()).is_empty());
    }

    #[test]
    fn wait_stats_read_out_the_way_the_panel_shows_them() {
        let mut w = WaitStats::default();
        assert_eq!(w.readout(), None);
        for _ in 0..3 {
            w.record(Source::Buffer, 0.0);
        }
        w.record(Source::Ready, 0.0);
        w.record(Source::Live, 0.34);
        w.record(Source::Deadline, 1.5);
        assert_eq!(w.shown(), 6);
        assert_eq!(w.ready_on_arrival(), 4);
        assert_eq!(
            w.readout().as_deref(),
            Some("ready on arrival 4/6 · longest wait 1.5 s")
        );
        assert!((w.mean_s() - (0.34 + 1.5) / 6.0).abs() < 1e-6);
    }
}
