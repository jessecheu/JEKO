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

/// Professor Pebble's hub tutorial, said before one of their lines from
/// the bank.
pub const PEBBLE_TUTORIAL: &[&str] = &[
    "Welcome to JEKO Beach! I'm Professor Pebble.",
    "Walk with W A S D. North, through the arch, is The Road.",
    "Message bottles on the boardwalk hold math problems. Stand by one and press E.",
    "Sea-grass on the deck hides monsters. Answer right to beat them; wrong answers cost hearts!",
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
