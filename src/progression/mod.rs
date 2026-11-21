pub mod upgrades;

use crate::llm::MathTask;
use std::collections::{BTreeMap, VecDeque};
use upgrades::{default_upgrades, Upgrade};

/// How a topic has gone this run (FR22): plain counters, no rendering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TopicStat {
    pub correct: u32,
    pub total: u32,
}

impl TopicStat {
    pub fn accuracy(&self) -> f32 {
        if self.total == 0 {
            1.0
        } else {
            self.correct as f32 / self.total as f32
        }
    }
}

/// Attempts a topic needs before it can count as weak, so one slip isn't a trend.
pub const WEAK_MIN_ATTEMPTS: u32 = 2;
/// A topic is weak below this accuracy.
pub const WEAK_BELOW: f32 = 0.75;
/// How many missed problems are kept for "similar practice".
pub const MISSED_KEPT: usize = 4;

/// The incentive loop for a session: a visible progress bar toward the next
/// level, a streak multiplier that rewards consecutive correct answers (so
/// stopping mid-streak feels costly), and XP-gated upgrades that give a
/// concrete reason to keep walking into the next trigger.
pub struct Progression {
    pub xp: u32,
    pub xp_to_next_level: u32,
    pub level: u32,
    pub lifetime_xp: u32,
    pub streak: u32,
    pub best_streak: u32,
    pub upgrades: Vec<Upgrade>,
    /// Accuracy per topic this run (FR22), keyed by the lowercase topic. Not
    /// saved between runs: it's the first piece of learner data the game
    /// would keep, which needs the owner's privacy decision first.
    pub topics: BTreeMap<String, TopicStat>,
    /// Problems the player got wrong, newest last, waiting to come back as
    /// similar ones (FR22).
    missed: VecDeque<MathTask>,
}

impl Progression {
    pub fn new() -> Self {
        Self {
            xp: 0,
            xp_to_next_level: 100,
            level: 1,
            lifetime_xp: 0,
            streak: 0,
            best_streak: 0,
            upgrades: default_upgrades(),
            topics: BTreeMap::new(),
            missed: VecDeque::new(),
        }
    }

    /// Counts one answer toward its topic. A wrong one is also remembered, so
    /// the studio can write similar practice.
    pub fn record_topic(&mut self, task: &MathTask, correct: bool) {
        let key = crate::content::lessons::normalize_topic(&task.topic);
        if key.is_empty() {
            return;
        }
        let stat = self.topics.entry(key).or_default();
        stat.total += 1;
        if correct {
            stat.correct += 1;
        } else {
            if self.missed.len() == MISSED_KEPT {
                self.missed.pop_front();
            }
            self.missed.push_back(task.clone());
        }
    }

    /// The topic the player is doing worst on: the lowest accuracy below
    /// `WEAK_BELOW` among topics with at least `WEAK_MIN_ATTEMPTS` tries, the
    /// one with more tries first on a tie. `None` when nothing is weak.
    pub fn weakest_topic(&self) -> Option<&str> {
        self.topics
            .iter()
            .filter(|(_, s)| s.total >= WEAK_MIN_ATTEMPTS && s.accuracy() < WEAK_BELOW)
            .min_by(|(_, a), (_, b)| {
                a.accuracy()
                    .total_cmp(&b.accuracy())
                    .then(b.total.cmp(&a.total))
            })
            .map(|(t, _)| t.as_str())
    }

    /// The oldest problem waiting for similar practice, without taking it.
    pub fn peek_missed(&self) -> Option<&MathTask> {
        self.missed.front()
    }

    /// The oldest problem waiting for similar practice, handed out once.
    pub fn take_missed(&mut self) -> Option<MathTask> {
        self.missed.pop_front()
    }

    pub fn missed_waiting(&self) -> usize {
        self.missed.len()
    }

    pub fn progress_fraction(&self) -> f32 {
        self.xp as f32 / self.xp_to_next_level as f32
    }

    pub fn streak_multiplier(&self) -> f32 {
        1.0 + (self.streak.min(10) as f32) * 0.1
    }

    /// Returns any upgrades newly unlocked by this answer, for the UI to flash a toast.
    pub fn record_correct(&mut self, base_xp: u32) -> Vec<Upgrade> {
        self.streak += 1;
        self.best_streak = self.best_streak.max(self.streak);

        let gained = (base_xp as f32 * self.streak_multiplier()).round() as u32;
        self.xp += gained;
        self.lifetime_xp += gained;

        while self.xp >= self.xp_to_next_level {
            self.xp -= self.xp_to_next_level;
            self.level += 1;
            self.xp_to_next_level = (self.xp_to_next_level as f32 * 1.3).round() as u32;
        }

        let mut newly_unlocked = Vec::new();
        for upgrade in self.upgrades.iter_mut() {
            if !upgrade.unlocked && self.lifetime_xp >= upgrade.unlock_xp {
                upgrade.unlocked = true;
                newly_unlocked.push(upgrade.clone());
            }
        }
        newly_unlocked
    }

    pub fn record_incorrect(&mut self) {
        self.streak = 0;
    }

    pub fn speed_multiplier(&self) -> f32 {
        self.upgrades
            .iter()
            .map(|u| u.speed_multiplier())
            .fold(1.0, f32::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(topic: &str) -> MathTask {
        MathTask {
            topic: topic.into(),
            problem: format!("a {topic} problem"),
            answer: 1.0,
            hint: String::new(),
            difficulty: 1,
            explanation: Vec::new(),
            expression: None,
        }
    }

    #[test]
    fn answers_count_per_topic_ignoring_case() {
        let mut p = Progression::new();
        p.record_topic(&task("Fractions"), true);
        p.record_topic(&task(" fractions"), false);
        p.record_topic(&task("addition"), true);
        assert_eq!(
            p.topics["fractions"],
            TopicStat {
                correct: 1,
                total: 2
            }
        );
        assert_eq!(p.topics["addition"].accuracy(), 1.0);
        p.record_topic(&task("  "), true);
        assert_eq!(p.topics.len(), 2, "a blank topic isn't tracked");
    }

    #[test]
    fn the_weakest_topic_needs_a_few_tries_and_a_poor_score() {
        let mut p = Progression::new();
        assert_eq!(p.weakest_topic(), None);
        p.record_topic(&task("division"), false);
        assert_eq!(p.weakest_topic(), None, "one slip isn't a trend");
        p.record_topic(&task("division"), false);
        assert_eq!(p.weakest_topic(), Some("division"));
        // A worse topic wins; a good one never shows.
        for _ in 0..3 {
            p.record_topic(&task("addition"), true);
        }
        p.record_topic(&task("fractions"), false);
        p.record_topic(&task("fractions"), false);
        p.record_topic(&task("fractions"), false);
        assert_eq!(
            p.weakest_topic(),
            Some("fractions"),
            "same 0%: more tries first"
        );
        for _ in 0..6 {
            p.record_topic(&task("fractions"), true);
        }
        assert_eq!(p.weakest_topic(), Some("division"));
    }

    #[test]
    fn missed_problems_come_back_once_oldest_first_and_only_a_few_are_kept() {
        let mut p = Progression::new();
        for i in 0..6 {
            let mut t = task("addition");
            t.problem = format!("p{i}");
            p.record_topic(&t, false);
        }
        assert_eq!(p.missed_waiting(), MISSED_KEPT);
        assert_eq!(p.take_missed().unwrap().problem, "p2");
        assert_eq!(p.take_missed().unwrap().problem, "p3");
        p.record_topic(&task("addition"), true);
        assert_eq!(p.missed_waiting(), 2, "a right answer isn't remembered");
    }
}
