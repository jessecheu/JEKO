//! Checks content files the way the game would load them, so a person or an
//! agent (FR30) can test what it wrote before the game sees it. Offline, needs
//! no key, and never writes anything.
//!
//! ```text
//! validate_content                      dialogue bank and content dir
//! validate_content --dialogue [PATH]    only the dialogue bank (default assets/dialogue.json)
//! validate_content --content [DIR]      only the content dir (default: the game's content dir)
//! ```
//!
//! Exits 0 when everything passes, 1 when anything is rejected, 2 on bad usage.

use jeko::content::{self, vet::vet_text, watcher::Scanner, ContentEvent};
use jeko::game::dialogue;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const DEFAULT_DIALOGUE: &str = "assets/dialogue.json";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut dialogue_path: Option<PathBuf> = None;
    let mut content_path: Option<PathBuf> = None;
    let (mut only_dialogue, mut only_content) = (false, false);

    let mut i = 0;
    while i < args.len() {
        let target = match args[i].as_str() {
            "--dialogue" => (&mut only_dialogue, &mut dialogue_path),
            "--content" => (&mut only_content, &mut content_path),
            other => {
                eprintln!("unknown argument {other}\n{USAGE}");
                return ExitCode::from(2);
            }
        };
        *target.0 = true;
        // An optional path follows, unless the next argument is another flag.
        if let Some(p) = args.get(i + 1).filter(|a| !a.starts_with("--")) {
            *target.1 = Some(PathBuf::from(p));
            i += 1;
        }
        i += 1;
    }

    let both = !only_dialogue && !only_content;
    let mut failures = 0;
    if both || only_dialogue {
        let path = dialogue_path.unwrap_or_else(|| PathBuf::from(DEFAULT_DIALOGUE));
        failures += check_dialogue(&path);
    }
    if both || only_content {
        let dir = content_path.unwrap_or_else(content::content_dir);
        failures += check_content(&dir);
    }

    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        eprintln!("{failures} problem(s)");
        ExitCode::FAILURE
    }
}

const USAGE: &str = "usage: validate_content [--dialogue [PATH]] [--content [DIR]]";

/// Returns the number of problems (0 or 1).
fn check_dialogue(path: &Path) -> usize {
    let json = match std::fs::read_to_string(path) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("FAIL {}: {e}", path.display());
            return 1;
        }
    };
    match dialogue::check_bank(&json) {
        Ok(bank) => {
            let lines: usize = bank.iter().map(|n| n.lines.len()).sum();
            println!(
                "ok   {}: {} NPCs, {lines} lines",
                path.display(),
                bank.len()
            );
            0
        }
        Err(e) => {
            eprintln!("FAIL {}: {e}", path.display());
            1
        }
    }
}

/// Loads every monster spec, lesson pack and tips file under `dir` with the
/// game's own loaders (the watcher's first scan), then holds each loaded monster's
/// text to the word blocklist, which the monster loader doesn't apply. Reports each
/// rejection.
/// Returns the number of rejected files.
fn check_content(dir: &Path) -> usize {
    if !dir.is_dir() {
        eprintln!("FAIL {}: not a directory", dir.display());
        return 1;
    }
    let (mut monsters, mut packs, mut tips, mut bad) = (0, 0, 0, 0);
    for event in Scanner::new(dir).scan() {
        match event {
            ContentEvent::Monster { spec, .. } => {
                monsters += 1;
                let texts = [
                    ("name", &spec.name),
                    ("blurb", &spec.blurb),
                    ("fact", &spec.fact),
                    ("theme topic", &spec.theme.topic),
                ];
                for (field, text) in texts {
                    if let Err(word) = vet_text(text) {
                        eprintln!(
                            "FAIL {}.json: {field} uses the blocked word \"{word}\"",
                            spec.id
                        );
                        bad += 1;
                    }
                }
            }
            ContentEvent::Lessons { .. } => packs += 1,
            ContentEvent::Tips(_) => tips += 1,
            ContentEvent::Invalid { file, error } => {
                eprintln!("FAIL {file}: {error}");
                bad += 1;
            }
            ContentEvent::Removed { .. } => {}
        }
    }
    println!(
        "{} {}: {monsters} monsters, {packs} lesson packs, {tips} tips files, {bad} rejected",
        if bad == 0 { "ok  " } else { "FAIL" },
        dir.display()
    );
    bad
}
