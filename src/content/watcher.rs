//! Hot reload for the content dir (FR21). A background thread rescans
//! `<dir>/monsters/*.json` about twice a second, compares each file's
//! modification time and length with the last scan, and parses and validates
//! whatever changed. Results go to the game over an `mpsc` channel, which
//! `Game::update` drains with `try_recv`, so a slow disk never holds up a frame.
//!
//! Polling needs no new dependency. The `notify` crate is the upgrade if the
//! dir ever gets big.

use super::monster_spec::{self, MonsterSpec};
use super::ContentEvent;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// How often the watcher rescans: well inside FR21's "about 2 s" from save to screen.
pub const POLL: Duration = Duration::from_millis(500);

/// One file's fingerprint at the last scan.
type Stamp = (Option<SystemTime>, u64);

/// The scan itself, apart from the thread, so tests can drive it.
pub struct Scanner {
    dir: PathBuf,
    seen: HashMap<PathBuf, Stamp>,
}

impl Scanner {
    pub fn new(content_dir: &Path) -> Self {
        Self {
            dir: content_dir.join("monsters"),
            seen: HashMap::new(),
        }
    }

    /// What changed since the last call: new and edited files (loaded or
    /// rejected) and deleted ones. A missing dir is just empty.
    pub fn scan(&mut self) -> Vec<ContentEvent> {
        let mut now: HashMap<PathBuf, Stamp> = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for e in entries.flatten() {
                let path = e.path();
                if path.extension().and_then(|x| x.to_str()) != Some("json") {
                    continue;
                }
                if let Ok(meta) = e.metadata() {
                    if meta.is_file() {
                        now.insert(path, (meta.modified().ok(), meta.len()));
                    }
                }
            }
        }

        let mut events = Vec::new();
        let mut changed: Vec<&PathBuf> = now
            .iter()
            .filter(|(p, stamp)| self.seen.get(*p) != Some(stamp))
            .map(|(p, _)| p)
            .collect();
        changed.sort();
        for path in changed {
            let reloaded = self.seen.contains_key(path);
            events.push(match load(path) {
                Ok(spec) => ContentEvent::Monster {
                    spec: Arc::new(spec),
                    reloaded,
                },
                Err(error) => ContentEvent::Invalid {
                    file: display(path),
                    error,
                },
            });
        }
        let mut gone: Vec<&PathBuf> = self.seen.keys().filter(|p| !now.contains_key(*p)).collect();
        gone.sort();
        for path in gone {
            events.push(ContentEvent::Removed { id: stem(path) });
        }
        self.seen = now;
        events
    }
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn display(path: &Path) -> String {
    let parent = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!(
        "{parent}/{}",
        path.file_name().unwrap_or_default().to_string_lossy()
    )
}

/// Reads and validates one file. Its `id` must match its file name, so two
/// files can never fight over one monster.
fn load(path: &Path) -> Result<MonsterSpec, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let spec = monster_spec::parse(&text)?;
    let stem = stem(path);
    if spec.id != stem {
        return Err(format!(
            "id {:?} doesn't match the file name {stem:?}",
            spec.id
        ));
    }
    Ok(spec)
}

/// Starts the watcher thread on `content_dir`. The first scan reports every
/// file already there. The thread stops when the receiver is dropped and it
/// next has something to send.
pub fn spawn(content_dir: PathBuf) -> Receiver<ContentEvent> {
    let (tx, rx) = mpsc::channel();
    let mut scanner = Scanner::new(&content_dir);
    std::thread::Builder::new()
        .name("content-watcher".into())
        .spawn(move || loop {
            for event in scanner.scan() {
                if tx.send(event).is_err() {
                    return;
                }
            }
            std::thread::sleep(POLL);
        })
        .expect("spawn the content watcher thread");
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_json(id: &str, name: &str) -> String {
        format!(
            r##"{{"id": "{id}", "name": "{name}",
                "body": {{"shape": "blob", "w": 40, "h": 30}},
                "palette": {{"main": "#88cc66", "accent": "#ffcc44", "belly": "#ddeebb"}},
                "eyes": {{"count": 2, "size": 4}}, "tier": 1}}"##
        )
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("jeko-watch-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("monsters")).unwrap();
        d
    }

    fn names(events: &[ContentEvent]) -> Vec<String> {
        events
            .iter()
            .map(|e| match e {
                ContentEvent::Monster { spec, reloaded } => {
                    format!("{}{}", if *reloaded { "re:" } else { "" }, spec.name)
                }
                ContentEvent::Removed { id } => format!("-{id}"),
                ContentEvent::Invalid { file, .. } => format!("!{file}"),
            })
            .collect()
    }

    #[test]
    fn a_missing_dir_is_empty() {
        let mut s = Scanner::new(Path::new("/definitely/not/here"));
        assert!(s.scan().is_empty());
    }

    #[test]
    fn loads_then_reloads_on_edit_and_reports_bad_files_and_deletes() {
        let d = temp_dir("cycle");
        let file = d.join("monsters/moss.json");
        std::fs::write(&file, spec_json("moss", "Moss")).unwrap();
        std::fs::write(d.join("monsters/notes.txt"), "ignored").unwrap();
        let mut s = Scanner::new(&d);
        assert_eq!(names(&s.scan()), ["Moss"]);
        assert!(s.scan().is_empty(), "nothing changed");

        // An edit (here it also changes the length) is picked up as a reload.
        std::fs::write(&file, spec_json("moss", "Mossier")).unwrap();
        assert_eq!(names(&s.scan()), ["re:Mossier"]);

        // A broken edit is reported, not loaded.
        std::fs::write(&file, "{ \"id\": \"moss\", oops").unwrap();
        assert_eq!(names(&s.scan()), ["!monsters/moss.json"]);

        // A file whose id doesn't match its name is refused.
        std::fs::write(d.join("monsters/other.json"), spec_json("moss", "Imposter")).unwrap();
        assert_eq!(names(&s.scan()), ["!monsters/other.json"]);

        std::fs::remove_file(&file).unwrap();
        assert_eq!(names(&s.scan()), ["-moss"]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_shipped_specs_all_load() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("content");
        let events = Scanner::new(&dir).scan();
        assert!(events.len() >= 2, "content/monsters ships a few specs");
        for e in &events {
            assert!(
                matches!(e, ContentEvent::Monster { .. }),
                "{:?}",
                names(std::slice::from_ref(e))
            );
        }
    }
}
