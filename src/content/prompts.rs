//! Prompts for the studio (FR21): what the model is asked to write, kept in
//! one place and in the same style as `llm::task_gen::build_prompt`. Plain
//! strings, so they're easy to read, tweak and test.

/// A shipped spec shown to the model as the exact shape to follow. It's the
/// same file the game loads, so the example can't drift from the format.
const EXAMPLE: &str = include_str!("../../content/monsters/mossmunch.json");

/// The values the format accepts, as the prompt lists them. A test loads a
/// spec with every one, so this list can't fall out of step with
/// `monster_spec`.
pub const SHAPES: &[&str] = &["blob", "shell", "dome", "segmented", "wave", "spiky_ball"];
pub const PARTS: &[&str] = &[
    "claw", "tentacle", "spike", "fin", "wing", "horn", "leg", "tail",
];
pub const SIDES: &[&str] = &["front", "back", "both"];
pub const LOOKS: &[&str] = &["cute", "grumpy", "sleepy"];
pub const ACCESSORIES: &[&str] = &["none", "crown", "mask", "helmet", "bow_tie", "leaf"];
pub const IDLES: &[&str] = &["bob", "scuttle", "float", "sway", "pulse"];
pub const ATTACKS: &[&str] = &["lunge", "zap", "spin", "slam", "spit"];
/// What a generated monster may use. `stun` and `steals_hat` stay with the
/// hand-written ones until the owner decides otherwise (FR21's open question).
pub const GENERATED_SPECIALS: &[&str] = &["none", "grows"];

/// The prompt for one new monster of `tier` (1 = a gentle starter, 4 = the
/// toughest regular). `topic` is a maths topic its questions should lean on,
/// `avoid` the names already taken. Returns `(system, user)`.
pub fn monster_prompt(tier: u32, topic: &str, avoid: &[String]) -> (String, String) {
    let list = |xs: &[&str]| xs.join(" | ");
    let system = format!(
        "You design friendly-but-spooky forest creatures for a children's maths adventure game \
(ages 7 to 11) set in a dark, moonlit forest. A creature is a small data file, not a picture: \
you choose its body shape, colours, eyes, parts and moves from fixed lists. Always respond with \
ONLY a single JSON object, no prose, no markdown fences, in exactly this shape:\n{EXAMPLE}\n\
Rules: \"id\" is a short lowercase word with only letters, digits, - or _. \"name\" is an invented \
name of at most 16 characters. \"blurb\" is one cheerful line of at most 80 characters. \"fact\" is \
one TRUE, well-known fact of at most 160 characters about the real animal, plant or natural \
phenomenon the creature is based on. Allowed values: body.shape: {shapes}; parts[].kind: {parts}; \
parts[].side: {sides}; eyes.look: {looks}; accessory: {accs}; idle: {idles}; attack: {attacks}; \
special: {specials}. body.w is 20 to 76 and body.h is 18 to 64. eyes.count is 1 to 3 and eyes.size \
is 2 to 8. At most 6 parts, each with count 1 to 4 and size 0.5 to 1.5. Colours are #rrggbb and \
should be bright enough to read on a dark forest night. The creature must be ORIGINAL: never a \
character, creature or look-alike from any existing game, film, show or book. Keep it kind: \
nothing scary beyond a spooky-cute look, nothing violent, no weapons, no real people.",
        shapes = list(SHAPES),
        parts = list(PARTS),
        sides = list(SIDES),
        looks = list(LOOKS),
        accs = list(ACCESSORIES),
        idles = list(IDLES),
        attacks = list(ATTACKS),
        specials = list(GENERATED_SPECIALS),
    );
    let feel = match tier {
        1 => "a gentle starter creature: small and round-ish, cute eyes",
        2 => "a lively creature: a little cheeky, with a couple of interesting parts",
        3 => "a tricky creature: bigger, with a striking silhouette and a grumpy or sleepy look",
        _ => "an imposing but still friendly-spooky creature: large, many parts, a crown or helmet",
    };
    let mut user = format!(
        "Design one new creature of tier {tier}: {feel}. Its battle questions will be about \
{topic}, so give it a \"theme\" of {{\"subject\": \"math\", \"topic\": \"{topic}\"}}. Set \
\"tier\": {tier}."
    );
    if !avoid.is_empty() {
        let names: Vec<&str> = avoid.iter().map(String::as_str).take(24).collect();
        user.push_str(&format!(
            " Do not reuse these names, and make it look different from them: {}.",
            names.join(", ")
        ));
    }
    (system, user)
}

/// The maths topics a monster can lean on, in rough order of difficulty.
pub const TOPICS: &[&str] = &[
    "addition",
    "subtraction",
    "multiplication",
    "division",
    "fractions",
    "place value",
    "perimeter and area",
    "word problems",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::monster_spec::parse;

    fn spec_with(field: &str, value: &str) -> String {
        let mut v: serde_json::Value = serde_json::from_str(EXAMPLE).unwrap();
        let path: Vec<&str> = field.split('.').collect();
        let mut at = &mut v;
        for k in &path[..path.len() - 1] {
            at = &mut at[*k];
        }
        at[path[path.len() - 1]] = serde_json::Value::String(value.into());
        v.to_string()
    }

    #[test]
    fn the_example_is_a_valid_spec() {
        assert!(parse(EXAMPLE).is_ok());
    }

    #[test]
    fn every_value_the_prompt_lists_is_accepted_by_the_parser() {
        for (field, values) in [
            ("body.shape", SHAPES),
            ("eyes.look", LOOKS),
            ("accessory", ACCESSORIES),
            ("idle", IDLES),
            ("attack", ATTACKS),
            ("special", GENERATED_SPECIALS),
        ] {
            for v in values {
                let json = spec_with(field, v);
                assert!(parse(&json).is_ok(), "{field} = {v}");
            }
        }
        for kind in PARTS {
            for side in SIDES {
                let mut v: serde_json::Value = serde_json::from_str(EXAMPLE).unwrap();
                v["parts"] = serde_json::json!([{"kind": kind, "side": side}]);
                assert!(parse(&v.to_string()).is_ok(), "{kind} {side}");
            }
        }
    }

    #[test]
    fn the_prompt_names_the_tier_the_topic_and_what_to_avoid() {
        let (system, user) =
            monster_prompt(3, "fractions", &["Mossmunch".into(), "Inkwisp".into()]);
        assert!(system.contains("ONLY a single JSON object"));
        assert!(system.contains("ORIGINAL"));
        assert!(system.contains("\"mossmunch\""));
        assert!(user.contains("tier 3") && user.contains("fractions"));
        assert!(user.contains("Mossmunch, Inkwisp"));
        let (_, plain) = monster_prompt(1, "addition", &[]);
        assert!(!plain.contains("Do not reuse"));
    }

    #[test]
    fn topics_are_short_enough_for_a_theme() {
        assert!(TOPICS
            .iter()
            .all(|t| t.len() <= crate::content::monster_spec::THEME_MAX));
    }
}
