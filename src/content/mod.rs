//! Content that lives outside the binary and can change while the game runs
//! (FR21): monsters defined as JSON specs in the content dir, loaded and
//! hot-reloaded by a background watcher. Plain Rust with no macroquad or
//! egui, like `config` and `llm`. `game` draws and fights what's here.

pub mod monster_spec;
pub mod vet;
pub mod watcher;

pub use monster_spec::{MonsterInfo, MonsterSpec};

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

/// What the watcher reports about one file.
#[derive(Debug)]
pub enum ContentEvent {
    /// A spec loaded. `reloaded` is true when the file was already known,
    /// i.e. it was edited while the game ran.
    Monster {
        spec: Arc<MonsterSpec>,
        reloaded: bool,
    },
    /// The file for `id` was deleted.
    Removed { id: String },
    /// A file failed to parse or validate. Whatever loaded from it before
    /// stays in the roster.
    Invalid { file: String, error: String },
}

/// Every spec monster that's loaded, by id. Sorted, so any future
/// choice made by hashing over it is stable.
#[derive(Default)]
pub struct Roster {
    monsters: BTreeMap<String, Arc<MonsterSpec>>,
}

impl Roster {
    /// Applies one event. A rejected file leaves the last good version in
    /// place.
    pub fn apply(&mut self, event: &ContentEvent) {
        match event {
            ContentEvent::Monster { spec, .. } => {
                self.monsters.insert(spec.id.clone(), spec.clone());
            }
            ContentEvent::Removed { id } => {
                self.monsters.remove(id);
            }
            ContentEvent::Invalid { .. } => {}
        }
    }

    pub fn get(&self, id: &str) -> Option<&Arc<MonsterSpec>> {
        self.monsters.get(id)
    }

    pub fn len(&self) -> usize {
        self.monsters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.monsters.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<MonsterSpec>> {
        self.monsters.values()
    }
}

/// Where the content dir is: `JEKO_CONTENT_DIR` if set (environment or
/// `.env`), else `content/` in the working directory if there is one, else
/// the `content/` folder of the checkout the binary was built from.
pub fn content_dir() -> PathBuf {
    let _ = dotenvy::dotenv();
    if let Some(d) = std::env::var_os("JEKO_CONTENT_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    let here = PathBuf::from("content");
    if here.is_dir() {
        return here;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("content")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bad_edit_keeps_the_last_good_version() {
        let spec = Arc::new(
            monster_spec::parse(
                r##"{"id":"a","name":"A","body":{"shape":"blob","w":30,"h":30},
                "palette":{"main":"#88cc66","accent":"#ffcc44","belly":"#ddeebb"},
                "eyes":{"count":1,"size":4},"tier":2}"##,
            )
            .unwrap(),
        );
        let mut r = Roster::default();
        r.apply(&ContentEvent::Monster {
            spec,
            reloaded: false,
        });
        r.apply(&ContentEvent::Invalid {
            file: "monsters/a.json".into(),
            error: "oops".into(),
        });
        assert_eq!(r.get("a").map(|s| s.name.as_str()), Some("A"));
        r.apply(&ContentEvent::Removed { id: "a".into() });
        assert!(r.is_empty());
    }
}
