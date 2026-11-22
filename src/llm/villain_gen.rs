//! The mad scientist's live lines (FR23, milestone B): the prompt for one
//! "beat" (a line, a mood and an action) and the first pass over the model's
//! reply. Plain Rust, no macroquad or egui, like `task_gen`.
//!
//! This module only shapes text: it pulls the JSON out, trims and cuts the
//! line, and lowercases the enum names. `game::villain::beat_line` maps those
//! names onto the game's own enums and runs the blocklist, so the rules about
//! what the doctor may do stay with the doctor.

use super::task_gen::extract_json_object;
use serde::Deserialize;

/// Run context sent with a cue. It describes the run, never the person playing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BeatContext {
    /// The cue's key, as in `assets/villain_lines.json` (`flask_right`, ...).
    pub cue: String,
    /// The number the cue is about (metres, streak, level), if it has one.
    pub number: Option<u32>,
    pub meters: u32,
    pub level: u32,
    pub streak: u32,
    pub hearts: u32,
    /// The doctor's last few lines, so the model doesn't repeat itself.
    pub recent: Vec<String>,
}

/// What the model is allowed to pick from. Kept here, in the prompt's words;
/// `game::villain` maps them and defaults anything else.
pub const MOODS: [&str; 5] = [
    "smug",
    "gleeful",
    "flustered",
    "scheming",
    "furious_but_funny",
];
pub const ACTIONS: [&str; 9] = [
    "pace", "tinker", "brew", "cackle", "scribble", "spy", "fluster", "gloat", "scurry",
];

/// A reply after the first pass: a non-empty line of at most `max_chars`,
/// and the mood and action names, lowercased, if the model gave them.
#[derive(Clone, Debug, PartialEq)]
pub struct RawBeat {
    pub line: String,
    pub mood: Option<String>,
    pub action: Option<String>,
}

/// What one cue means, in a sentence for the prompt.
fn describe(cue: &str, n: Option<u32>) -> String {
    let n = n.unwrap_or(0);
    match cue {
        "first_sighting" => "The player has just walked through the gate and seen you for the first time. Introduce yourself.".into(),
        "milestone" => format!("The player has walked {n} metres up the road."),
        "flask_right" => "The player just solved one of your maths flasks correctly.".into(),
        "flask_wrong" => "The player just got one of your maths flasks wrong. Gloat about your own plan, never about the player.".into(),
        "battle_start" => "One of your monsters is about to battle the player with maths questions.".into(),
        "battle_won" => "The player just beat one of your monsters.".into(),
        "player_hurt" => "The player just lost some health.".into(),
        "streak" => format!("The player has answered {n} questions in a row correctly."),
        "level_up" => format!("The player just reached level {n}."),
        "low_health" => "The player is low on health. Be a cartoon villain, not a cruel one.".into(),
        _ => "Nothing has happened for a while. Mutter about your inventions.".into(),
    }
}

/// The `(system, user)` prompt for one beat.
pub fn build_prompt(ctx: &BeatContext, max_chars: usize) -> (String, String) {
    let system = format!(
        "You write lines for Dr. Nix Nullwell, an original, comic mad scientist in a maths game \
for young children. They are a gangly, over-caffeinated inventor in brass goggles and a scorched \
lab coat who wants to turn every number in the woods into zero so nobody ever has to do sums. They \
brag about their inventions, fall apart when the player does well, and are bad at maths themself. \
Rules: silly and kind, never scary or mean; mock themself and their inventions, never the player; \
no real people, brands or characters from other games, films or books; no violence. \
Reply with ONLY one JSON object, no prose and no markdown: \
{{\"line\": string, \"mood\": string, \"action\": string}}. \"line\" is one short spoken line of \
at most {max_chars} characters. \"mood\" is one of: {moods}. \"action\" is one of: {actions}.",
        moods = MOODS.join(", "),
        actions = ACTIONS.join(", "),
    );
    let mut user = format!(
        "{} The run so far: {} metres walked, level {}, a streak of {}, {} hearts left.",
        describe(&ctx.cue, ctx.number),
        ctx.meters,
        ctx.level,
        ctx.streak,
        ctx.hearts
    );
    if !ctx.recent.is_empty() {
        user.push_str(" Don't repeat these recent lines: ");
        user.push_str(
            &ctx.recent
                .iter()
                .map(|l| format!("\"{l}\""))
                .collect::<Vec<_>>()
                .join(", "),
        );
        user.push('.');
    }
    (system, user)
}

#[derive(Deserialize)]
struct Reply {
    line: String,
    #[serde(default)]
    mood: Option<String>,
    #[serde(default)]
    action: Option<String>,
}

/// The first pass over a reply. `None` when there's no JSON object, no
/// `line`, or the line is empty once trimmed. A long line is cut at a word
/// boundary to `max_chars`, with an ellipsis.
pub fn parse_reply(raw: &str, max_chars: usize) -> Option<RawBeat> {
    let reply: Reply = serde_json::from_str(extract_json_object(raw)?).ok()?;
    let line = reply
        .line
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c| c == '"' || c == '\'')
        .to_string();
    if line.is_empty() {
        return None;
    }
    let norm = |s: Option<String>| {
        s.map(|s| s.trim().to_lowercase().replace([' ', '-'], "_"))
            .filter(|s| !s.is_empty())
    };
    Some(RawBeat {
        line: cut(&line, max_chars),
        mood: norm(reply.mood),
        action: norm(reply.action),
    })
}

/// `s` cut to at most `max` characters at a word boundary, with "…".
fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for word in s.split(' ') {
        let next = if out.is_empty() {
            word.chars().count()
        } else {
            out.chars().count() + 1 + word.chars().count()
        };
        if next + 1 > max {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.is_empty() {
        out = s.chars().take(max - 1).collect();
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(cue: &str) -> BeatContext {
        BeatContext {
            cue: cue.into(),
            number: Some(200),
            meters: 230,
            level: 2,
            streak: 3,
            hearts: 5,
            recent: vec!["Old line!".into()],
        }
    }

    #[test]
    fn the_prompt_names_the_cue_the_run_and_the_choices() {
        let (system, user) = build_prompt(&ctx("milestone"), 90);
        assert!(system.contains("90 characters") && system.contains("furious_but_funny"));
        assert!(system.contains("never the player"));
        assert!(user.contains("200 metres") && user.contains("level 2"));
        assert!(user.contains("\"Old line!\""));
    }

    #[test]
    fn a_chatty_reply_is_cleaned_up() {
        let raw = "Sure! {\"line\": \"  Curses!\\n  My   fraction   fizzled! \", \"mood\": \"Furious But Funny\", \"action\": \"FLUSTER\"} hope that helps";
        let b = parse_reply(raw, 90).unwrap();
        assert_eq!(b.line, "Curses! My fraction fizzled!");
        assert_eq!(b.mood.as_deref(), Some("furious_but_funny"));
        assert_eq!(b.action.as_deref(), Some("fluster"));
    }

    #[test]
    fn hostile_replies_are_refused_or_cut() {
        assert_eq!(parse_reply("no json here", 90), None);
        assert_eq!(parse_reply("{\"mood\": \"smug\"}", 90), None);
        assert_eq!(parse_reply("{\"line\": \"   \"}", 90), None);
        assert_eq!(parse_reply("{\"line\": 7}", 90), None);
        let long = format!("{{\"line\": \"{}\"}}", "zero ".repeat(60));
        let b = parse_reply(&long, 90).unwrap();
        assert!(b.line.chars().count() <= 90 && b.line.ends_with('…'));
        assert_eq!((b.mood, b.action), (None, None));
        let word = format!("{{\"line\": \"{}\"}}", "z".repeat(200));
        assert_eq!(parse_reply(&word, 90).unwrap().line.chars().count(), 90);
    }
}
