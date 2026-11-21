//! The field guide (FR22): beating a monster unlocks its card. Pure data, no
//! drawing: `ui::field_guide_panel` is the view. A card is a name, a line about
//! the creature and a true fact about the real animal or thing it's drawn from.
//!
//! The collection lasts for the session and survives a new run, like the
//! roster. It isn't saved to disk (nothing is yet: FR14).

use super::monster::{MonsterKind, Species};
use crate::content::Roster;
use std::collections::BTreeSet;

/// One card as the panel shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuideCard {
    pub id: String,
    pub name: String,
    pub blurb: String,
    /// Empty when the creature has no fact on file.
    pub fact: String,
    /// Beaten at least once. A card that isn't found shows as "???".
    pub found: bool,
}

#[derive(Default)]
pub struct FieldGuide {
    found: BTreeSet<String>,
}

impl Species {
    /// Its key in the field guide: stable between runs, and unique across
    /// built-ins and specs.
    pub fn guide_id(&self) -> String {
        match self {
            Species::Builtin(k) => format!("builtin:{}", k.name().to_lowercase().replace(' ', "-")),
            Species::Spec(s) => s.id.clone(),
        }
    }
}

impl FieldGuide {
    /// Records a beaten monster. True the first time, when a new card unlocks.
    pub fn unlock(&mut self, species: &Species) -> bool {
        self.found.insert(species.guide_id())
    }

    pub fn found_count(&self) -> usize {
        self.found.len()
    }

    /// Every card there is: the seven built-ins, then the specs in the roster
    /// by id, each marked found or not.
    pub fn cards(&self, roster: &Roster) -> Vec<GuideCard> {
        let builtins = MonsterKind::ALL.iter().map(|k| {
            let (blurb, fact) = k.card();
            let id = Species::Builtin(*k).guide_id();
            GuideCard {
                found: self.found.contains(&id),
                id,
                name: k.name().to_string(),
                blurb: blurb.to_string(),
                fact: fact.to_string(),
            }
        });
        let specs = roster.iter().map(|s| GuideCard {
            found: self.found.contains(&s.id),
            id: s.id.clone(),
            name: s.name.clone(),
            blurb: s.blurb.clone(),
            fact: s.fact.clone(),
        });
        builtins.chain(specs).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{monster_spec, ContentEvent};
    use std::sync::Arc;

    fn roster_with(ids: &[&str]) -> Roster {
        let mut r = Roster::default();
        for id in ids {
            let spec = monster_spec::parse(&format!(
                r##"{{"id":"{id}","name":"{id}","blurb":"A blurb.","fact":"A fact.",
                "body":{{"shape":"blob","w":40,"h":30}},
                "palette":{{"main":"#88cc66","accent":"#ffcc44","belly":"#ddeebb"}},
                "eyes":{{"count":2,"size":4}},"tier":1}}"##
            ))
            .unwrap();
            r.apply(&ContentEvent::Monster {
                spec: Arc::new(spec),
                reloaded: false,
            });
        }
        r
    }

    #[test]
    fn every_builtin_has_a_card_with_a_fact() {
        for k in MonsterKind::ALL {
            let (blurb, fact) = k.card();
            assert!(!blurb.is_empty() && !fact.is_empty(), "{}", k.name());
        }
    }

    #[test]
    fn a_card_unlocks_once_and_stays() {
        let mut g = FieldGuide::default();
        let coco = Species::Builtin(MonsterKind::CocoKnight);
        assert!(g.unlock(&coco));
        assert!(!g.unlock(&coco), "the second win isn't a new card");
        assert_eq!(g.found_count(), 1);
        let cards = g.cards(&Roster::default());
        assert_eq!(cards.len(), MonsterKind::ALL.len());
        let found: Vec<&str> = cards
            .iter()
            .filter(|c| c.found)
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(found, ["Coco Knight"]);
    }

    #[test]
    fn spec_monsters_get_cards_too_after_the_built_ins() {
        let roster = roster_with(&["b-two", "a-one"]);
        let mut g = FieldGuide::default();
        let spec = Species::Spec(roster.get("b-two").unwrap().clone());
        assert!(g.unlock(&spec));
        let cards = g.cards(&roster);
        assert_eq!(cards.len(), MonsterKind::ALL.len() + 2);
        let tail: Vec<(&str, bool)> = cards[MonsterKind::ALL.len()..]
            .iter()
            .map(|c| (c.id.as_str(), c.found))
            .collect();
        assert_eq!(tail, [("a-one", false), ("b-two", true)]);
        assert_eq!(cards.last().unwrap().fact, "A fact.");
    }

    #[test]
    fn ids_never_collide_between_builtins_and_specs() {
        let ids: BTreeSet<String> = MonsterKind::ALL
            .iter()
            .map(|k| Species::Builtin(*k).guide_id())
            .collect();
        assert_eq!(ids.len(), MonsterKind::ALL.len());
        assert!(
            ids.iter().all(|i| i.starts_with("builtin:")),
            "a spec id can't have a colon"
        );
    }
}
