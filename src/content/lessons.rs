//! Lesson packs (FR22): a batch of practice problems on one topic and level,
//! written by the studio ahead of need (or by a person) as
//! `lessons/<topic>-<level>.json` in the content dir, and loaded by the
//! watcher into the question buffer.
//!
//! Every problem in a pack has been through `llm::expr::verify`: its `answer`
//! equals the value of its `expression`. A problem that fails is dropped, not
//! fixed, and a pack with nothing left is rejected whole. What's on disk is
//! only ever the verified problems, and the watcher checks them again when it
//! loads the file, so a hand edit can't smuggle a wrong answer in.

use crate::llm::expr;
use crate::llm::task_gen::extract_json_array;
use crate::llm::MathTask;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const PROBLEM_MAX: usize = 240;
pub const HINT_MAX: usize = 120;
pub const STEP_MAX: usize = 120;
pub const STEPS_MAX: usize = 4;
pub const PACK_MAX: usize = 12;
pub const TOPIC_MAX: usize = 24;

/// A problem as the model writes it and as the pack file stores it.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Problem {
    problem: String,
    answer: f64,
    #[serde(default)]
    expression: Option<String>,
    #[serde(default)]
    hint: String,
    #[serde(default)]
    explanation: Vec<String>,
}

/// The pack file.
#[derive(Debug, Serialize, Deserialize)]
struct PackFile {
    topic: String,
    level: u32,
    problems: Vec<Problem>,
}

/// A loaded pack: verified problems ready for the question buffer.
#[derive(Clone, Debug, PartialEq)]
pub struct Pack {
    pub topic: String,
    pub level: u32,
    pub tasks: Vec<MathTask>,
    /// Problems turned down while loading.
    pub dropped: usize,
}

/// Topics compare ignoring case and extra spaces, and are short.
pub fn normalize_topic(topic: &str) -> String {
    let words: Vec<&str> = topic.split_whitespace().collect();
    let flat: String = words.join(" ").to_lowercase();
    flat.chars()
        .filter(|c| !c.is_control())
        .take(TOPIC_MAX)
        .collect()
}

/// One line of text: whitespace collapsed, control characters dropped. `None`
/// if it's longer than `max`: a problem cut short would be a different
/// problem, so it's turned down instead.
fn tidy(s: &str, max: usize) -> Option<String> {
    let words: Vec<&str> = s.split_whitespace().collect();
    let flat: String = words
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    (flat.chars().count() <= max).then_some(flat)
}

/// Why one problem was turned down, or the task it makes.
fn make_task(p: Problem, topic: &str, level: u32) -> Result<MathTask, String> {
    let problem = tidy(&p.problem, PROBLEM_MAX).ok_or("problem text too long")?;
    let hint = tidy(&p.hint, HINT_MAX).ok_or("hint too long")?;
    let mut steps = Vec::new();
    for step in p.explanation.iter().take(STEPS_MAX) {
        steps.push(tidy(step, STEP_MAX).ok_or("a step is too long")?);
    }
    let task = MathTask {
        topic: topic.to_string(),
        problem,
        answer: p.answer,
        hint,
        difficulty: level,
        explanation: steps,
        expression: p.expression.map(|e| e.trim().to_string()),
    };
    expr::verify(&task).map_err(|why| format!("{why:?}"))?;
    for text in std::iter::once(&task.problem)
        .chain(std::iter::once(&task.hint))
        .chain(&task.explanation)
    {
        super::studio::vet_text(text).map_err(|w| format!("off-limits word: {w}"))?;
    }
    Ok(task)
}

fn build(file: PackFile) -> Result<Pack, String> {
    let topic = normalize_topic(&file.topic);
    if topic.is_empty() {
        return Err("the pack has no topic".into());
    }
    let level = file.level.clamp(1, 5);
    let total = file.problems.len();
    let tasks: Vec<MathTask> = file
        .problems
        .into_iter()
        .take(PACK_MAX)
        .filter_map(|p| make_task(p, &topic, level).ok())
        .collect();
    if tasks.is_empty() {
        return Err(format!("none of the {total} problems checked out"));
    }
    Ok(Pack {
        dropped: total.min(PACK_MAX) - tasks.len(),
        topic,
        level,
        tasks,
    })
}

/// Parses and verifies a pack file. `Err` when it isn't a pack, or when no
/// problem in it passes.
pub fn parse_pack(json: &str) -> Result<Pack, String> {
    let file: PackFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
    build(file)
}

/// Turns the model's reply (a JSON array of problems) into a pack file in
/// `dir/lessons/`, keeping only the problems that verify. Returns the file
/// written and the pack. Nothing is written if nothing passes.
pub fn finish_pack(
    raw: &str,
    dir: &Path,
    topic: &str,
    level: u32,
    n: usize,
) -> Result<(PathBuf, Pack), String> {
    let slice = extract_json_array(raw).ok_or("no JSON array in the reply")?;
    let problems: Vec<Problem> =
        serde_json::from_str(slice).map_err(|e| format!("bad JSON: {e}"))?;
    let file = PackFile {
        topic: topic.to_string(),
        level,
        problems: problems.into_iter().take(n.clamp(1, PACK_MAX)).collect(),
    };
    let pack = build(file)?;

    // Write only what passed, so the file on disk is always checkable.
    let kept = PackFile {
        topic: pack.topic.clone(),
        level: pack.level,
        problems: pack
            .tasks
            .iter()
            .map(|t| Problem {
                problem: t.problem.clone(),
                answer: t.answer,
                expression: t.expression.clone(),
                hint: t.hint.clone(),
                explanation: t.explanation.clone(),
            })
            .collect(),
    };
    let lessons = dir.join("lessons");
    std::fs::create_dir_all(&lessons).map_err(|e| e.to_string())?;
    let slug: String = pack
        .topic
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "topic" } else { slug };
    let base = format!("{slug}-{}", pack.level);
    let mut name = format!("{base}.json");
    let mut k = 2;
    while lessons.join(&name).exists() {
        name = format!("{base}-{k}.json");
        k += 1;
    }
    let path = lessons.join(&name);
    let tmp = lessons.join(format!("{name}.tmp"));
    let text = serde_json::to_string_pretty(&kept).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok((path, pack))
}

/// A made-up reply for the stand-in model: `n` verified problems from the
/// offline bank, so the studio's lesson path runs with no key.
pub fn fake_pack_json(n: usize) -> String {
    let bank = crate::llm::fallback_bank();
    let problems: Vec<Problem> = bank
        .into_iter()
        .cycle()
        .take(n)
        .map(|t| Problem {
            problem: t.problem,
            answer: t.answer,
            expression: t.expression,
            hint: t.hint,
            explanation: t.explanation,
        })
        .collect();
    serde_json::to_string(&problems).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("jeko-lessons-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    const GOOD: &str = r#"[
        {"problem": "A bag has 12 red and 8 blue marbles. Shared among 4 kids, how many each?",
         "answer": 5, "expression": "(12 + 8) / 4", "hint": "Add, then share.",
         "explanation": ["12 + 8 = 20.", "20 / 4 = 5."]},
        {"problem": "What is half of 18?", "answer": 9, "expression": "18 / 2",
         "hint": "Split into two equal parts.", "explanation": ["18 / 2 = 9."]}
    ]"#;

    #[test]
    fn a_good_reply_becomes_a_pack_file_of_verified_problems() {
        let d = temp_dir("good");
        let (path, pack) = finish_pack(&format!("Here: {GOOD}"), &d, "Fractions ", 2, 8).unwrap();
        assert_eq!(
            (pack.topic.as_str(), pack.level, pack.tasks.len()),
            ("fractions", 2, 2)
        );
        assert!(pack
            .tasks
            .iter()
            .all(|t| t.topic == "fractions" && t.difficulty == 2));
        assert!(path.ends_with("lessons/fractions-2.json"));
        // What's on disk loads back to the same problems.
        let loaded = parse_pack(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.tasks.len(), 2);
        assert_eq!(loaded.tasks[0].expression.as_deref(), Some("(12 + 8) / 4"));
        // A second pack on the same topic and level doesn't overwrite the first.
        let (second, _) = finish_pack(GOOD, &d, "fractions", 2, 8).unwrap();
        assert!(second.ends_with("lessons/fractions-2-2.json"));
        assert!(!d.join("lessons/fractions-2.json.tmp").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_wrong_answer_key_is_dropped_and_never_written() {
        let d = temp_dir("wrong");
        let raw = r#"[
            {"problem": "What is 7 + 5?", "answer": 13, "expression": "7 + 5", "explanation": ["7 + 5."]},
            {"problem": "What is 6 x 7?", "answer": 42, "expression": "6 * 7", "explanation": ["6 * 7 = 42."]}
        ]"#;
        let (path, pack) = finish_pack(raw, &d, "multiplication", 2, 8).unwrap();
        assert_eq!((pack.tasks.len(), pack.dropped), (1, 1));
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("6 x 7") && !text.contains("7 + 5"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn problems_that_cant_be_checked_are_dropped() {
        let raw = r#"[
            {"problem": "No expression at all.", "answer": 4},
            {"problem": "Not arithmetic.", "answer": 8, "expression": "2 ^ 3"},
            {"problem": "Empty step.", "answer": 2, "expression": "1 + 1", "explanation": ["1 + 1 = 2.", ""]},
            {"problem": "", "answer": 2, "expression": "1 + 1"},
            {"problem": "Answer isn't a number.", "answer": null, "expression": "1 + 1"}
        ]"#;
        assert!(finish_pack(raw, &temp_dir("unchecked"), "addition", 1, 8).is_err());
    }

    #[test]
    fn off_limits_words_and_overlong_text_are_dropped() {
        let long = "a".repeat(PROBLEM_MAX + 1);
        let raw = format!(
            r#"[
            {{"problem": "The pirate will kill 3 of the 5 sailors. How many are left?", "answer": 2, "expression": "5 - 3"}},
            {{"problem": "{long}", "answer": 2, "expression": "1 + 1"}},
            {{"problem": "What is 1 + 1?", "answer": 2, "expression": "1 + 1", "hint": "Count on."}}
        ]"#
        );
        let (_, pack) = finish_pack(&raw, &temp_dir("vet"), "addition", 1, 8).unwrap();
        assert_eq!((pack.tasks.len(), pack.dropped), (1, 2));
        assert_eq!(pack.tasks[0].problem, "What is 1 + 1?");
    }

    #[test]
    fn bad_replies_write_nothing() {
        let d = temp_dir("bad");
        for raw in [
            "Sorry, no.",
            "[not json]",
            "{\"problem\": \"x\"}",
            "[]",
            "[1, 2]",
        ] {
            assert!(finish_pack(raw, &d, "addition", 1, 8).is_err(), "{raw}");
        }
        assert!(
            !d.join("lessons").exists()
                || std::fs::read_dir(d.join("lessons")).unwrap().count() == 0
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_pack_is_capped_and_the_level_clamped() {
        let many: Vec<String> = (0..40)
            .map(|i| {
                format!(
                    r#"{{"problem": "What is {i} + 1?", "answer": {}, "expression": "{i} + 1"}}"#,
                    i + 1
                )
            })
            .collect();
        let raw = format!("[{}]", many.join(","));
        let (_, pack) = finish_pack(&raw, &temp_dir("cap"), "addition", 99, 40).unwrap();
        assert_eq!((pack.tasks.len(), pack.level), (PACK_MAX, 5));
    }

    #[test]
    fn a_hand_edited_wrong_answer_is_caught_on_load() {
        let json = r#"{"topic": "addition", "level": 1, "problems": [
            {"problem": "What is 2 + 2?", "answer": 5, "expression": "2 + 2"},
            {"problem": "What is 3 + 3?", "answer": 6, "expression": "3 + 3"}]}"#;
        let pack = parse_pack(json).unwrap();
        assert_eq!((pack.tasks.len(), pack.dropped), (1, 1));
        assert!(parse_pack(
            r#"{"topic": "x", "level": 1, "problems": [{"problem": "p", "answer": 1}]}"#
        )
        .is_err());
        assert!(parse_pack(r#"{"topic": " ", "level": 1, "problems": []}"#).is_err());
        assert!(parse_pack("{").is_err());
    }

    #[test]
    fn the_stand_in_models_packs_always_pass() {
        let (_, pack) =
            finish_pack(&fake_pack_json(8), &temp_dir("fake"), "word problems", 3, 8).unwrap();
        assert_eq!(pack.tasks.len(), 8);
    }

    #[test]
    fn topics_are_normalised() {
        assert_eq!(normalize_topic("  Place   Value "), "place value");
        assert_eq!(normalize_topic(&"x".repeat(80)).len(), TOPIC_MAX);
    }
}
