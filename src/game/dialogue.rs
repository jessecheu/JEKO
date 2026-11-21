//! NPC dialogue (FR9): a speaker, a few lines, and a typewriter reveal.
//! E finishes the current line, then moves to the next, then closes.

use serde::Deserialize;

/// Characters revealed per second.
const TYPE_RATE: f32 = 45.0;

/// One NPC's lines as stored in `assets/dialogue.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct NpcLines {
    pub npc: String,
    pub lines: Vec<String>,
}

pub fn load_bank() -> Vec<NpcLines> {
    serde_json::from_str(include_str!("../../assets/dialogue.json"))
        .expect("assets/dialogue.json must be valid")
}

/// Rules for a bank of NPC lines, enforced by `check_bank` for `gen_content`
/// and `validate_content` (FR30). `load_bank` is stricter in one way only: it
/// panics at startup on JSON that doesn't parse.
pub const MIN_LINES: usize = 3;
pub const MAX_LINES: usize = 12;
pub const MAX_LINE_CHARS: usize = 140;
/// The one NPC the game looks up by name (`Game` starts their hub dialogue).
pub const REQUIRED_NPC: &str = "Professor Pebble";

/// Parses and checks a dialogue bank before it's written to, or accepted as,
/// `assets/dialogue.json`: an array of `{npc, lines}` where every NPC has a
/// name, 3 to 12 non-empty lines of at most 140 characters, no line passes the
/// studio's word blocklist, names are unique, and Professor Pebble is there.
/// The first problem found is the error. The game compiles this file in and
/// panics on a bad one, so a check that fails here saves a broken build.
pub fn check_bank(json: &str) -> Result<Vec<NpcLines>, String> {
    let bank: Vec<NpcLines> =
        serde_json::from_str(json).map_err(|e| format!("not a list of {{npc, lines}}: {e}"))?;
    let mut seen: Vec<&str> = Vec::new();
    for npc in &bank {
        let name = npc.npc.trim();
        if name.is_empty() {
            return Err("an NPC has no name".into());
        }
        if seen.contains(&name) {
            return Err(format!("{name} appears twice"));
        }
        seen.push(name);
        if !(MIN_LINES..=MAX_LINES).contains(&npc.lines.len()) {
            return Err(format!(
                "{name} has {} lines, want {MIN_LINES} to {MAX_LINES}",
                npc.lines.len()
            ));
        }
        for line in &npc.lines {
            let n = line.chars().count();
            if line.trim().is_empty() {
                return Err(format!("{name} has an empty line"));
            }
            if n > MAX_LINE_CHARS {
                return Err(format!(
                    "{name}: a line is {n} characters, the limit is {MAX_LINE_CHARS}: \"{line}\""
                ));
            }
            if let Err(word) = crate::content::studio::vet_text(line) {
                return Err(format!(
                    "{name}: \"{line}\" uses the blocked word \"{word}\""
                ));
            }
        }
    }
    if !seen.contains(&REQUIRED_NPC) {
        return Err(format!("{REQUIRED_NPC} is missing"));
    }
    Ok(bank)
}

/// Professor Pebble's hub tutorial, said before one of their lines from
/// the bank.
pub const PEBBLE_TUTORIAL: &[&str] = &[
    "Welcome to JEKO Woods! I'm Professor Pebble. Warm your hands by the fire.",
    "Walk with W A S D. North, through the iron gate, is The Road. It leads to the old manor across the lake.",
    "Glowing potion flasks on the road hold math problems. Stand by one and press E.",
    "Ferns on the road hide monsters. Answer right to beat them; wrong answers cost hearts!",
];

#[derive(Debug, Clone)]
pub struct Dialogue {
    pub speaker: String,
    pub lines: Vec<String>,
    pub line: usize,
    /// Characters of the current line revealed so far.
    shown: f32,
}

impl Dialogue {
    pub fn new(speaker: impl Into<String>, lines: Vec<String>) -> Self {
        Self {
            speaker: speaker.into(),
            lines,
            line: 0,
            shown: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.shown += dt * TYPE_RATE;
    }

    fn current(&self) -> &str {
        self.lines.get(self.line).map(String::as_str).unwrap_or("")
    }

    /// True once the whole current line is on screen.
    pub fn line_done(&self) -> bool {
        self.shown as usize >= self.current().chars().count()
    }

    /// The part of the current line typed out so far.
    pub fn visible(&self) -> String {
        self.current().chars().take(self.shown as usize).collect()
    }

    /// E: finishes the line if it's still typing, otherwise moves on.
    /// Returns false once the last line has been dismissed.
    pub fn advance(&mut self) -> bool {
        if !self.line_done() {
            self.shown = self.current().chars().count() as f32;
            return true;
        }
        self.line += 1;
        self.shown = 0.0;
        self.line < self.lines.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bank_has_professor_pebble() {
        assert!(load_bank().iter().any(|n| n.npc == "Professor Pebble"));
    }

    #[test]
    fn the_shipped_bank_passes_the_check() {
        let json = include_str!("../../assets/dialogue.json");
        check_bank(json).expect("assets/dialogue.json must pass check_bank");
    }

    fn bank_with(npc: &str, lines: &[&str]) -> String {
        let lines: Vec<String> = lines.iter().map(|l| format!("{l:?}")).collect();
        format!(r#"[{{"npc":{npc:?},"lines":[{}]}}]"#, lines.join(","))
    }

    #[test]
    fn check_bank_rejects_hostile_banks() {
        let ok = ["One.", "Two.", "Three."];
        assert!(check_bank(&bank_with(REQUIRED_NPC, &ok)).is_ok());
        let cases: Vec<(String, &str)> = vec![
            ("not json".into(), "not a list"),
            (r#"{"npc":"x"}"#.into(), "not a list"),
            ("[]".into(), "missing"),
            (bank_with("Someone Else", &ok), "missing"),
            (bank_with(REQUIRED_NPC, &ok[..2]), "2 lines"),
            (bank_with(REQUIRED_NPC, &["a"; 13]), "13 lines"),
            (
                bank_with(REQUIRED_NPC, &["One.", "  ", "Three."]),
                "empty line",
            ),
            (bank_with("  ", &ok), "no name"),
            (
                bank_with(REQUIRED_NPC, &["One.", "Two.", &"x".repeat(141)]),
                "141 characters",
            ),
            (
                bank_with(REQUIRED_NPC, &["One.", "Two.", "Fear the Pikachu!"]),
                "pikachu",
            ),
            (
                format!(
                    "[{},{}]",
                    &bank_with(REQUIRED_NPC, &ok)[1..bank_with(REQUIRED_NPC, &ok).len() - 1],
                    &bank_with(REQUIRED_NPC, &ok)[1..bank_with(REQUIRED_NPC, &ok).len() - 1]
                ),
                "twice",
            ),
        ];
        for (json, want) in cases {
            let err = check_bank(&json).expect_err(&json);
            assert!(err.to_lowercase().contains(want), "{json:.60} -> {err}");
        }
    }

    #[test]
    fn typewriter_reveals_then_advances_then_closes() {
        let mut d = Dialogue::new("P", vec!["Hello".into(), "Bye".into()]);
        assert_eq!(d.visible(), "");
        d.update(2.0 / TYPE_RATE);
        assert_eq!(d.visible(), "He");
        assert!(d.advance(), "first E finishes the line");
        assert_eq!(d.visible(), "Hello");
        assert!(d.advance(), "second E moves to the next line");
        assert_eq!(d.visible(), "");
        d.update(1.0);
        assert!(d.line_done());
        assert!(!d.advance(), "E on the last full line closes");
    }
}
