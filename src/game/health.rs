//! Player HP (FR13), counted in half-hearts so damage can take half a
//! heart. Kept apart from `Progression`: XP and HP are separate concerns.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Health {
    /// Half-hearts left.
    pub current: u32,
    /// Half-hearts when full (2 per heart).
    pub max: u32,
}

/// Starting hearts.
pub const START_HEARTS: u32 = 5;

impl Health {
    pub fn new(hearts: u32) -> Self {
        Self {
            current: hearts * 2,
            max: hearts * 2,
        }
    }

    /// Takes `halves` half-hearts, never going below 0. Returns what was
    /// actually lost.
    pub fn damage(&mut self, halves: u32) -> u32 {
        let lost = halves.min(self.current);
        self.current -= lost;
        lost
    }

    /// Restores `halves` half-hearts, never going above `max`.
    pub fn heal(&mut self, halves: u32) {
        self.current = (self.current + halves).min(self.max);
    }

    pub fn refill(&mut self) {
        self.current = self.max;
    }

    pub fn is_dead(&self) -> bool {
        self.current == 0
    }

    /// Hearts to draw (rounded up).
    pub fn hearts(&self) -> u32 {
        self.max.div_ceil(2)
    }
}

impl Default for Health {
    fn default() -> Self {
        Self::new(START_HEARTS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_full() {
        let h = Health::new(5);
        assert_eq!((h.current, h.max, h.hearts()), (10, 10, 5));
        assert!(!h.is_dead());
    }

    #[test]
    fn damage_clamps_at_zero() {
        let mut h = Health::new(1);
        assert_eq!(h.damage(1), 1);
        assert_eq!(h.damage(5), 1);
        assert_eq!(h.current, 0);
        assert!(h.is_dead());
        assert_eq!(h.damage(1), 0);
    }

    #[test]
    fn heal_clamps_at_max() {
        let mut h = Health::new(3);
        h.damage(3);
        h.heal(1);
        assert_eq!(h.current, 4);
        h.heal(100);
        assert_eq!(h.current, 6);
    }

    #[test]
    fn refill_restores_max() {
        let mut h = Health::new(2);
        h.damage(3);
        h.refill();
        assert_eq!(h.current, h.max);
    }
}
