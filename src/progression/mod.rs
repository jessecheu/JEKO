pub mod upgrades;

use upgrades::{default_upgrades, Upgrade};

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
        }
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
