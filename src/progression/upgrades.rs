#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeKind {
    SpeedBoost,
}

#[derive(Debug, Clone)]
pub struct Upgrade {
    pub kind: UpgradeKind,
    pub name: &'static str,
    pub description: &'static str,
    pub unlock_xp: u32,
    pub unlocked: bool,
}

/// Vertical-slice scope ships one upgrade. Natural next additions: a dash
/// (movement upgrade), a cosmetic skin (identity/status incentive), and a
/// second zone unlock (progress-gated content) — each slots in here as
/// another `Upgrade` without touching the progression logic.
pub fn default_upgrades() -> Vec<Upgrade> {
    vec![Upgrade {
        kind: UpgradeKind::SpeedBoost,
        name: "Quickstep Boots",
        description: "+25% move speed",
        unlock_xp: 100,
        unlocked: false,
    }]
}

impl Upgrade {
    pub fn speed_multiplier(&self) -> f32 {
        match (self.kind, self.unlocked) {
            (UpgradeKind::SpeedBoost, true) => 1.25,
            _ => 1.0,
        }
    }
}
