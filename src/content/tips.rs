//! Professor Pebble's tips (FR22): a few kind, specific lines about a topic
//! the player has been missing, written by the studio into
//! `tips/<topic>.json` and loaded by the watcher into the dialogue. Short and
//! vetted like everything else the model writes.

use crate::llm::task_gen::extract_json_array;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const TIP_MAX: usize = 160;
pub const TIPS_MAX: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tips {
    pub npc: String,
    pub topic: String,
    pub lines: Vec<String>,
}

/// The prompt for tips on `topic`, spoken by Professor Pebble. `(system, user)`.
pub fn tips_prompt(topic: &str) -> (String, String) {
    let system = "You write short lines of dialogue for Professor Pebble, a warm, encouraging \
teacher in a children's maths adventure game (ages 7 to 11). Always respond with ONLY a single \
JSON array of strings, no prose, no markdown fences."
        .to_string();
    let user = format!(
        "Write 3 different one-sentence tips (each at most 140 characters) that help a child who \
keeps getting {topic} problems wrong. Each tip is a concrete trick or way to think about it, \
said kindly in Professor Pebble's voice. No scolding, nothing scary."
    );
    (system, user)
}

fn tidy(line: &str) -> Option<String> {
    let flat: String = line
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    (!flat.is_empty() && flat.chars().count() <= TIP_MAX && super::studio::vet_text(&flat).is_ok())
        .then_some(flat)
}

/// Parses and checks a tips file: lines that are empty, too long or off-limits
/// are dropped, and at least one must remain.
pub fn parse(json: &str) -> Result<Tips, String> {
    let mut t: Tips = serde_json::from_str(json).map_err(|e| e.to_string())?;
    t.lines = t
        .lines
        .iter()
        .filter_map(|l| tidy(l))
        .take(TIPS_MAX)
        .collect();
    t.npc = t.npc.trim().to_string();
    t.topic = super::lessons::normalize_topic(&t.topic);
    if t.npc.is_empty() || t.topic.is_empty() {
        return Err("tips need an npc and a topic".into());
    }
    if t.lines.is_empty() {
        return Err("no usable tip lines".into());
    }
    Ok(t)
}

/// Turns the model's reply (a JSON array of strings) into
/// `dir/tips/<topic>.json`. A second batch on the same topic replaces the first.
pub fn finish(raw: &str, dir: &Path, npc: &str, topic: &str) -> Result<PathBuf, String> {
    let slice = extract_json_array(raw).ok_or("no JSON array in the reply")?;
    let lines: Vec<String> = serde_json::from_str(slice).map_err(|e| format!("bad JSON: {e}"))?;
    let tips = parse(
        &serde_json::to_string(&Tips {
            npc: npc.into(),
            topic: topic.into(),
            lines,
        })
        .map_err(|e| e.to_string())?,
    )?;
    let slug: String = tips
        .topic
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let folder = dir.join("tips");
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let name = format!("{}.json", slug.trim_matches('-'));
    let tmp = folder.join(format!("{name}.tmp"));
    std::fs::write(
        &tmp,
        serde_json::to_string_pretty(&tips).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let path = folder.join(name);
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("jeko-tips-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn a_good_reply_becomes_a_tips_file_that_loads_back() {
        let d = temp_dir("good");
        let raw = r#"Sure! ["Draw the pieces first.", "  Cut it in   equal parts. ", "Check by adding back."]"#;
        let path = finish(raw, &d, "Professor Pebble", "Fractions").unwrap();
        assert!(path.ends_with("tips/fractions.json"));
        let t = parse(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(
            (t.npc.as_str(), t.topic.as_str()),
            ("Professor Pebble", "fractions")
        );
        assert_eq!(
            t.lines,
            [
                "Draw the pieces first.",
                "Cut it in equal parts.",
                "Check by adding back."
            ]
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn bad_lines_are_dropped_and_an_empty_result_is_an_error() {
        let long = "x".repeat(TIP_MAX + 1);
        let raw = format!(
            r#"["", "{long}", "That will kill your score.", "Keep going!", "a", "b", "c", "d"]"#
        );
        let t = parse(&format!(
            r#"{{"npc": "P", "topic": "t", "lines": {}}}"#,
            raw
        ))
        .unwrap();
        assert_eq!(t.lines.len(), TIPS_MAX);
        assert_eq!(t.lines[0], "Keep going!");
        let d = temp_dir("bad");
        for raw in ["no", "[]", r#"["kill it"]"#, "[1]"] {
            assert!(finish(raw, &d, "P", "t").is_err(), "{raw}");
        }
        assert!(finish(r#"["ok"]"#, &d, " ", "t").is_err(), "no npc");
        assert!(finish(r#"["ok"]"#, &d, "P", " ").is_err(), "no topic");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_prompt_names_the_topic() {
        let (system, user) = tips_prompt("fractions");
        assert!(system.contains("Professor Pebble") && user.contains("fractions"));
    }
}
