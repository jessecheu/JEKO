//! The mad scientist's live lines (FR23, milestones B and C): what the doctor
//! can do and feel, what the game asks the model for, and the pure checks on
//! what comes back. Plain Rust, no macroquad or egui (invariant 10), so it's
//! unit-tested here; `game::villain` draws the result and
//! `CerebrasClient::request_villain_async` makes the call.
//!
//! A reply is one JSON **beat**: `{"line", "mood", "action"}`, plus a `scheme`
//! when the doctor is announcing one. Nothing from the model reaches the screen
//! unchecked: an unknown mood or action becomes a default, an over-long line is
//! cut at a word, and a line (or a scheme name) with a blocked word, a link or
//! no words at all is turned down, so the caller says a line from the reviewed
//! bank instead.

use crate::llm::task_gen::extract_json_object;
use serde::Deserialize;
use serde_json::Value;

/// The longest a line may be: what the speech bubble holds. The line bank uses
/// the same cap.
pub const MAX_LINE_CHARS: usize = 90;
/// The longest a scheme's name may be (it's painted on a sign on the cart).
pub const MAX_SCHEME_CHARS: usize = 32;

/// The word check a line must pass: `content::vet::vet_text`, passed in so
/// `llm` doesn't depend on `content`. `Err(word)` turns a line down.
pub type Vet = fn(&str) -> Result<(), String>;

/// What the doctor is doing. A closed set: a line names one, and idle time
/// picks among the first six.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VillainAction {
    /// Strolls across the top of the road, coat flapping.
    Pace,
    /// Bent over the cart with a wrench, sparks flying.
    Tinker,
    /// Pours the flask: a puff of coloured smoke.
    Brew,
    /// Leans back, shoulders shaking: "MWAHAHA!"
    Cackle,
    /// Writes a wrong sum on the chalkboard.
    Scribble,
    /// A telescope pointed at the player.
    Spy,
    /// Frizz up, flask fizzing over: the player got something right.
    Fluster,
    /// Rubs gloves together: the plan is going well.
    Gloat,
    /// Holds the flask up high to show off a new creation (FR23 C).
    Unveil,
    /// Dashes ahead pushing the cart when the player gains on them.
    Scurry,
    /// A soot puff, and they're there.
    Appear,
    /// A soot puff, and they're gone.
    Vanish,
}

impl VillainAction {
    pub const ALL: [VillainAction; 12] = [
        VillainAction::Pace,
        VillainAction::Tinker,
        VillainAction::Brew,
        VillainAction::Cackle,
        VillainAction::Scribble,
        VillainAction::Spy,
        VillainAction::Fluster,
        VillainAction::Gloat,
        VillainAction::Unveil,
        VillainAction::Scurry,
        VillainAction::Appear,
        VillainAction::Vanish,
    ];

    pub fn name(self) -> &'static str {
        match self {
            VillainAction::Pace => "pace",
            VillainAction::Tinker => "tinker",
            VillainAction::Brew => "brew",
            VillainAction::Cackle => "cackle",
            VillainAction::Scribble => "scribble",
            VillainAction::Spy => "spy",
            VillainAction::Fluster => "fluster",
            VillainAction::Gloat => "gloat",
            VillainAction::Unveil => "unveil",
            VillainAction::Scurry => "scurry",
            VillainAction::Appear => "appear",
            VillainAction::Vanish => "vanish",
        }
    }

    /// The action a line may ask for. The soot puffs are the game's to
    /// choose: they follow where the player is, not what was said.
    pub fn speakable(self) -> bool {
        !matches!(self, VillainAction::Appear | VillainAction::Vanish)
    }

    fn from_name(s: &str) -> Option<VillainAction> {
        let s = normalize(s);
        Self::ALL.into_iter().find(|a| a.name() == s)
    }
}

/// How the doctor feels; colours the flask.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mood {
    Smug,
    Gleeful,
    Flustered,
    Scheming,
    FuriousButFunny,
}

impl Mood {
    pub const ALL: [Mood; 5] = [
        Mood::Smug,
        Mood::Gleeful,
        Mood::Flustered,
        Mood::Scheming,
        Mood::FuriousButFunny,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Mood::Smug => "smug",
            Mood::Gleeful => "gleeful",
            Mood::Flustered => "flustered",
            Mood::Scheming => "scheming",
            Mood::FuriousButFunny => "furious_but_funny",
        }
    }

    /// What the doctor does when a line names no action they can do.
    pub fn default_action(self) -> VillainAction {
        match self {
            Mood::Smug => VillainAction::Gloat,
            Mood::Gleeful => VillainAction::Cackle,
            Mood::Flustered => VillainAction::Fluster,
            Mood::Scheming => VillainAction::Scribble,
            Mood::FuriousButFunny => VillainAction::Fluster,
        }
    }

    fn from_name(s: &str) -> Option<Mood> {
        let s = normalize(s);
        Self::ALL.into_iter().find(|m| m.name() == s)
    }
}

/// A scheme's cosmetic effect (FR23 C). A closed set: none of them changes a
/// rule (invariant 7). The scheme's *topic* is what steers questions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gadget {
    /// A low green fog that drifts off the cart across the top of the road.
    FogMachine,
    /// Soap-like bubbles that float up off the cart.
    BubbleRain,
    /// A pennant on a pole over the cart, with the scheme's name.
    Banner,
    /// The flask glows in the scheme's colour.
    GlowFlask,
}

impl Gadget {
    pub const ALL: [Gadget; 4] = [
        Gadget::FogMachine,
        Gadget::BubbleRain,
        Gadget::Banner,
        Gadget::GlowFlask,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Gadget::FogMachine => "fog_machine",
            Gadget::BubbleRain => "bubble_rain",
            Gadget::Banner => "banner",
            Gadget::GlowFlask => "glow_flask",
        }
    }

    fn from_name(s: &str) -> Option<Gadget> {
        let s = normalize(s);
        Self::ALL.into_iter().find(|g| g.name() == s)
    }
}

/// A scheme the doctor announces (FR23 C): a name, the maths topic their
/// flasks will ask about for a while, and a cosmetic gadget.
#[derive(Clone, Debug, PartialEq)]
pub struct Scheme {
    pub name: String,
    pub topic: String,
    pub gadget: Gadget,
}

/// One thing the doctor says and does.
#[derive(Clone, Debug, PartialEq)]
pub struct Beat {
    pub line: String,
    pub mood: Mood,
    pub action: VillainAction,
    /// Set only on a scheme announcement.
    pub scheme: Option<Scheme>,
}

/// How the run is going, for the prompt. It never carries anything about
/// the person playing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunContext {
    pub meters: u32,
    pub level: u32,
    pub streak: u32,
    /// Hearts in half-hearts, and the most there can be.
    pub health: u32,
    pub max_health: u32,
    pub last_monster: Option<String>,
    pub last_topic: Option<String>,
    /// The scheme running now, by name.
    pub scheme: Option<String>,
}

/// One beat to write: the cue's key in `assets/villain_lines.json`, what
/// happened in a sentence, the mood to fall back on, and the run.
#[derive(Clone, Debug, PartialEq)]
pub struct BeatAsk {
    pub cue: &'static str,
    pub happened: String,
    pub mood: Mood,
    pub ctx: RunContext,
    /// For a scheme announcement: the topics it may pick (they fit the
    /// player's level). Empty for every other cue.
    pub scheme_topics: Vec<String>,
}

/// What the game asks for: one beat, or both reactions to a flask that's open
/// (right and wrong, in one call), so the doctor can react on the frame the
/// answer lands.
#[derive(Clone, Debug, PartialEq)]
pub enum VillainAsk {
    One(BeatAsk),
    Pair { right: BeatAsk, wrong: BeatAsk },
}

#[derive(Clone, Debug, PartialEq)]
pub enum VillainReply {
    One(Beat),
    Pair { right: Beat, wrong: Beat },
}

/// Why a reply was turned down. The caller says a bank line instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BeatError {
    /// No JSON object in the reply, or not the shape asked for.
    Shape(String),
    /// No line, or nothing left of it once tidied.
    NoLine,
    /// A blocked word, or a link.
    Blocked(String),
    /// A scheme announcement whose scheme is missing or unusable.
    BadScheme(String),
}

impl std::fmt::Display for BeatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BeatError::Shape(why) => write!(f, "not a beat: {why}"),
            BeatError::NoLine => write!(f, "no line"),
            BeatError::Blocked(word) => write!(f, "uses {word:?}"),
            BeatError::BadScheme(why) => write!(f, "bad scheme: {why}"),
        }
    }
}

impl std::error::Error for BeatError {}

// ---------------------------------------------------------------------------
// The prompt

const CHARACTER: &str = "You write what Dr. Nix Nullwell says in a children's maths game. \
Dr. Nullwell is a gangly, over-caffeinated inventor in brass goggles and a scorched lab coat \
who pushes a rolling lab cart up a forest road at night. Their plan is to turn every number \
in the woods into zero, so nobody ever has to do sums again. They are comic, never scary: \
they brag about their inventions and their creatures (the monsters on the road), fall apart \
when the player gets things right, and are bad at maths themselves (their chalkboard says \
2 + 2 = 5). They are always just out of reach up the road.";

const RULES: &str = "Rules:\n\
- An original character. Never mention real people, brands, or characters from other games, \
films, shows or books.\n\
- Silly and warm, never scary, rude or mean. Make fun of yourself, your plan and your \
inventions, NEVER the player: no name-calling, never say the player is slow, bad or wrong \
as a person. When the player gets something wrong, gloat about your own plan instead.\n\
- Words a seven-year-old knows. No violence, no scary threats.\n\
- One line, at most 90 characters, no emoji, no quotes around it, no hashtags or links.\n\
- Reply with JSON only, no other text.";

/// The system and user messages for `ask`.
pub fn build_prompt(ask: &VillainAsk) -> (String, String) {
    let moods: Vec<&str> = Mood::ALL.iter().map(|m| m.name()).collect();
    let actions: Vec<&str> = VillainAction::ALL
        .iter()
        .filter(|a| a.speakable())
        .map(|a| a.name())
        .collect();
    let beat_shape =
        format!("{{\"line\": string, \"mood\": one of {moods:?}, \"action\": one of {actions:?}}}");
    let system = format!("{CHARACTER}\n\n{RULES}");
    let user = match ask {
        VillainAsk::One(a) => {
            let mut user = format!(
                "What just happened: {}\nThe run so far: {}\n",
                a.happened,
                describe(&a.ctx)
            );
            if a.scheme_topics.is_empty() {
                user.push_str(&format!("Reply with one JSON object: {beat_shape}"));
            } else {
                let gadgets: Vec<&str> = Gadget::ALL.iter().map(|g| g.name()).collect();
                user.push_str(&format!(
                    "The line announces the scheme by its name. Reply with one JSON object: \
{{\"line\": string, \"mood\": one of {moods:?}, \"action\": one of {actions:?}, \
\"scheme\": {{\"name\": a short funny name of at most {MAX_SCHEME_CHARS} characters, \
\"topic\": one of {:?}, \"gadget\": one of {gadgets:?}}}}}",
                    a.scheme_topics
                ));
            }
            user
        }
        VillainAsk::Pair { right, .. } => {
            let topic = right
                .ctx
                .last_topic
                .as_deref()
                .map_or(String::new(), |t| format!(" about {t}"));
            format!(
                "The player has just opened one of your potion flasks and is working on a \
maths problem{topic}.\nThe run so far: {}\nWrite two lines now: one to say if they get it \
right (you are flustered, your plan is foiled) and one if they get it wrong (gloat about \
your plan, not about the player). Reply with one JSON object: \
{{\"right\": {beat_shape}, \"wrong\": {beat_shape}}}",
                describe(&right.ctx)
            )
        }
    };
    (system, user)
}

fn describe(ctx: &RunContext) -> String {
    let mut s = format!(
        "{} m up the road, level {}, {} right in a row, {} of {} hearts left",
        ctx.meters,
        ctx.level,
        ctx.streak,
        ctx.health as f32 / 2.0,
        ctx.max_health as f32 / 2.0
    );
    if let Some(m) = &ctx.last_monster {
        s.push_str(&format!(", the last creature they met was {m}"));
    }
    if let Some(t) = &ctx.last_topic {
        s.push_str(&format!(", the last problem was about {t}"));
    }
    if let Some(name) = &ctx.scheme {
        s.push_str(&format!(", your scheme right now is {name}"));
    }
    s.push('.');
    s
}

// ---------------------------------------------------------------------------
// Checking the reply

/// Turns the model's reply to `ask` into beats, or says why not. Pure.
pub fn parse_reply(ask: &VillainAsk, raw: &str, vet: Vet) -> Result<VillainReply, BeatError> {
    let json = extract_json_object(raw).ok_or_else(|| BeatError::Shape("no JSON object".into()))?;
    let v: Value = serde_json::from_str(json).map_err(|e| BeatError::Shape(e.to_string()))?;
    match ask {
        VillainAsk::One(a) => parse_beat(a, &v, vet).map(VillainReply::One),
        VillainAsk::Pair { right, wrong } => Ok(VillainReply::Pair {
            right: parse_beat(right, &v["right"], vet)?,
            wrong: parse_beat(wrong, &v["wrong"], vet)?,
        }),
    }
}

/// One beat from a JSON value: the line tidied and cut to fit, the mood and
/// action defaulted when they're unknown, the scheme checked when one is asked
/// for.
pub fn parse_beat(ask: &BeatAsk, v: &Value, vet: Vet) -> Result<Beat, BeatError> {
    if !v.is_object() {
        return Err(BeatError::Shape("not an object".into()));
    }
    let line = v["line"].as_str().ok_or(BeatError::NoLine)?;
    let line = tidy_line(line, MAX_LINE_CHARS).ok_or(BeatError::NoLine)?;
    check_words(&line, vet)?;
    let mood = v["mood"]
        .as_str()
        .and_then(Mood::from_name)
        .unwrap_or(ask.mood);
    let action = v["action"]
        .as_str()
        .and_then(VillainAction::from_name)
        .filter(|a| a.speakable())
        .unwrap_or_else(|| mood.default_action());
    let scheme = if ask.scheme_topics.is_empty() {
        None
    } else {
        Some(parse_scheme(&v["scheme"], &ask.scheme_topics, vet)?)
    };
    Ok(Beat {
        line,
        mood,
        action,
        scheme,
    })
}

/// A scheme: a name that fits the sign and passes the word check, a topic from
/// `topics` (anything else would steer questions somewhere the player's level
/// doesn't go, so it's turned down), and a gadget, defaulted when unknown.
fn parse_scheme(v: &Value, topics: &[String], vet: Vet) -> Result<Scheme, BeatError> {
    let name = v["name"]
        .as_str()
        .ok_or_else(|| BeatError::BadScheme("no name".into()))?;
    let name = tidy_line(name, usize::MAX).ok_or_else(|| BeatError::BadScheme("no name".into()))?;
    if name.chars().count() > MAX_SCHEME_CHARS {
        return Err(BeatError::BadScheme(format!("{name:?} is too long")));
    }
    check_words(&name, vet)?;
    let asked = v["topic"].as_str().unwrap_or_default();
    let topic = topics
        .iter()
        .find(|t| t.trim().eq_ignore_ascii_case(asked.trim()))
        .ok_or_else(|| BeatError::BadScheme(format!("topic {asked:?} isn't one offered")))?
        .clone();
    let gadget = v["gadget"]
        .as_str()
        .and_then(Gadget::from_name)
        .unwrap_or_else(|| fallback_gadget(&topic));
    Ok(Scheme {
        name,
        topic,
        gadget,
    })
}

fn check_words(text: &str, vet: Vet) -> Result<(), BeatError> {
    let lower = text.to_lowercase();
    if lower.contains("http") || lower.contains("www.") || lower.contains(".com") {
        return Err(BeatError::Blocked("a link".into()));
    }
    vet(text).map_err(BeatError::Blocked)
}

/// One clean line: whitespace collapsed, control characters, markup and
/// placeholder braces dropped, wrapping quotes taken off, and cut at a word
/// with an ellipsis if it's longer than `max` characters. `None` if no
/// letters are left.
pub fn tidy_line(raw: &str, max: usize) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .filter(|c| !c.is_control() && !matches!(c, '*' | '`' | '#' | '{' | '}' | '<' | '>'))
        .collect();
    let mut line = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    for (open, close) in [('"', '"'), ('“', '”'), ('\'', '\'')] {
        if line.len() >= 2 && line.starts_with(open) && line.ends_with(close) {
            line = line[open.len_utf8()..line.len() - close.len_utf8()]
                .trim()
                .to_string();
        }
    }
    if !line.chars().any(char::is_alphabetic) {
        return None;
    }
    if line.chars().count() <= max {
        return Some(line);
    }
    // Cut at the last space that leaves room for the ellipsis; a single huge
    // word is cut where it must be.
    let room = max.saturating_sub(1);
    let mut out = String::new();
    for word in line.split(' ') {
        let next = if out.is_empty() {
            word.chars().count()
        } else {
            out.chars().count() + 1 + word.chars().count()
        };
        if next > room {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.is_empty() {
        out = line.chars().take(room).collect();
    }
    let out = out.trim_end_matches([',', ';', ':', ' ', '-']).to_string();
    Some(format!("{out}…"))
}

fn normalize(s: &str) -> String {
    s.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

// ---------------------------------------------------------------------------
// Offline schemes and the stand-in model

/// The scheme the doctor announces when there's no live one (no key, a bad
/// reply, the rate cap): one per maths topic, so it always fits the level.
pub fn fallback_scheme(topic: &str) -> Scheme {
    let name = match topic.trim().to_ascii_lowercase().as_str() {
        "addition" => "The Great Plus-Sign Heist",
        "subtraction" => "Operation Minus Mist",
        "multiplication" => "The Times-Table Tangle",
        "division" => "Project Split-Pea Soup",
        "fractions" => "Operation Fraction Fog",
        "place value" => "The Wandering Digits Plot",
        "perimeter and area" => "The Squashed Squares Scheme",
        "word problems" => "The Riddle Muddle",
        _ => "The Zero-Everything Machine",
    };
    Scheme {
        name: name.to_string(),
        topic: topic.to_string(),
        gadget: fallback_gadget(topic),
    }
}

fn fallback_gadget(topic: &str) -> Gadget {
    match topic.trim().to_ascii_lowercase().as_str() {
        "subtraction" | "fractions" => Gadget::FogMachine,
        "addition" | "place value" => Gadget::BubbleRain,
        "multiplication" | "perimeter and area" => Gadget::Banner,
        _ => Gadget::GlowFlask,
    }
}

/// What the stand-in model (`JEKO_DEV=fakemodel`) answers: a reply in the real
/// shape, marked so a screenshot shows it came from the live path.
pub fn fake_reply(ask: &VillainAsk) -> String {
    fn beat(a: &BeatAsk) -> Value {
        let mut v = serde_json::json!({
            "line": format!("(stand-in) A live {} line at {} m! Curses!", a.cue.replace('_', " "), a.ctx.meters),
            "mood": a.mood.name(),
            "action": a.mood.default_action().name(),
        });
        if let Some(topic) = a.scheme_topics.first() {
            v["line"] = Value::from("(stand-in) Behold: Operation Stand-In!");
            v["scheme"] = serde_json::json!({
                "name": "Operation Stand-In",
                "topic": topic,
                "gadget": fallback_gadget(topic).name(),
            });
        }
        v
    }
    match ask {
        VillainAsk::One(a) => beat(a).to_string(),
        VillainAsk::Pair { right, wrong } => {
            serde_json::json!({"right": beat(right), "wrong": beat(wrong)}).to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_bad_words(text: &str) -> Result<(), String> {
        text.split(|c: char| !c.is_alphanumeric())
            .map(str::to_lowercase)
            .find(|w| w == "stupid" || w == "mario")
            .map_or(Ok(()), Err)
    }

    fn ask(cue: &'static str) -> BeatAsk {
        BeatAsk {
            cue,
            happened: "The player just got one right.".into(),
            mood: Mood::Flustered,
            ctx: RunContext {
                meters: 120,
                level: 2,
                streak: 3,
                health: 5,
                max_health: 6,
                last_monster: Some("Pinchy".into()),
                last_topic: Some("fractions".into()),
                scheme: None,
            },
            scheme_topics: Vec::new(),
        }
    }

    fn scheme_ask() -> BeatAsk {
        BeatAsk {
            scheme_topics: vec!["fractions".into(), "division".into()],
            mood: Mood::Scheming,
            ..ask("scheme")
        }
    }

    fn one(raw: &str) -> Result<Beat, BeatError> {
        match parse_reply(&VillainAsk::One(ask("flask_right")), raw, no_bad_words)? {
            VillainReply::One(b) => Ok(b),
            _ => unreachable!(),
        }
    }

    #[test]
    fn a_good_beat_parses_from_a_chatty_reply() {
        let b = one(
            "Sure! ```json\n{\"line\": \"My finest fraction, foiled!\", \"mood\": \"flustered\", \"action\": \"fluster\"}\n```",
        )
        .unwrap();
        assert_eq!(b.line, "My finest fraction, foiled!");
        assert_eq!(
            (b.mood, b.action),
            (Mood::Flustered, VillainAction::Fluster)
        );
        assert!(b.scheme.is_none());
    }

    #[test]
    fn unknown_moods_and_actions_become_defaults() {
        // An unknown mood falls back on the cue's; an unknown action on the mood's.
        let b = one(r#"{"line": "Bah!", "mood": "grumpy", "action": "explode"}"#).unwrap();
        assert_eq!(
            (b.mood, b.action),
            (Mood::Flustered, VillainAction::Fluster)
        );
        // Missing both.
        let b = one(r#"{"line": "Bah!"}"#).unwrap();
        assert_eq!(b.mood, Mood::Flustered);
        // The actions the game alone chooses are never taken from a line.
        for a in ["appear", "vanish"] {
            let b = one(&format!(
                r#"{{"line": "Bah!", "mood": "smug", "action": "{a}"}}"#
            ))
            .unwrap();
            assert_eq!(b.action, VillainAction::Gloat, "{a}");
        }
        // Spelling is forgiven: case, spaces and hyphens.
        let b =
            one(r#"{"line": "Bah!", "mood": "Furious But-Funny", "action": " CACKLE "}"#).unwrap();
        assert_eq!(
            (b.mood, b.action),
            (Mood::FuriousButFunny, VillainAction::Cackle)
        );
    }

    #[test]
    fn hostile_lines_are_cut_tidied_or_turned_down() {
        // Over-long: cut at a word with an ellipsis, never over the cap.
        let long = "Behold ".repeat(30);
        let b = one(&format!(r#"{{"line": "{long}"}}"#)).unwrap();
        assert!(b.line.chars().count() <= MAX_LINE_CHARS, "{}", b.line);
        assert!(b.line.ends_with('…') && b.line.starts_with("Behold Behold"));
        // One enormous word is cut where it must be.
        let word = "a".repeat(200);
        let b = one(&format!(r#"{{"line": "{word}"}}"#)).unwrap();
        assert_eq!(b.line.chars().count(), MAX_LINE_CHARS);
        // Markup, newlines, braces and wrapping quotes go.
        let b = one(r#"{"line": "  \"**Mwaha!**\n {m} metres?!\"  "}"#).unwrap();
        assert_eq!(b.line, "Mwaha! m metres?!");
        // Blocked words and links are turned down, not shown.
        assert!(matches!(
            one(r#"{"line": "You are STUPID"}"#),
            Err(BeatError::Blocked(_))
        ));
        assert!(matches!(
            one(r#"{"line": "Mario sends regards"}"#),
            Err(BeatError::Blocked(_))
        ));
        assert!(matches!(
            one(r#"{"line": "see http://x.y"}"#),
            Err(BeatError::Blocked(_))
        ));
        // No line, an empty one, or one with no words.
        assert_eq!(one(r#"{"mood": "smug"}"#), Err(BeatError::NoLine));
        assert_eq!(one(r#"{"line": "   "}"#), Err(BeatError::NoLine));
        assert_eq!(one(r#"{"line": "!!! ???"}"#), Err(BeatError::NoLine));
        assert_eq!(one(r#"{"line": 42}"#), Err(BeatError::NoLine));
        // Not JSON, or not an object.
        assert!(matches!(one("I'd rather not."), Err(BeatError::Shape(_))));
        assert!(matches!(one("{not json}"), Err(BeatError::Shape(_))));
    }

    #[test]
    fn a_pair_needs_both_halves() {
        let pair = VillainAsk::Pair {
            right: ask("flask_right"),
            wrong: BeatAsk {
                mood: Mood::Gleeful,
                ..ask("flask_wrong")
            },
        };
        let ok = r#"{"right": {"line": "Noooo!"}, "wrong": {"line": "Hee hee, my plan!"}}"#;
        let VillainReply::Pair { right, wrong } = parse_reply(&pair, ok, no_bad_words).unwrap()
        else {
            panic!("a pair")
        };
        assert_eq!(
            (right.line.as_str(), right.mood),
            ("Noooo!", Mood::Flustered)
        );
        assert_eq!(
            (wrong.line.as_str(), wrong.mood),
            ("Hee hee, my plan!", Mood::Gleeful)
        );
        let half = r#"{"right": {"line": "Noooo!"}}"#;
        assert!(parse_reply(&pair, half, no_bad_words).is_err());
        let rude = r#"{"right": {"line": "Noooo!"}, "wrong": {"line": "stupid"}}"#;
        assert!(parse_reply(&pair, rude, no_bad_words).is_err());
    }

    #[test]
    fn a_scheme_is_checked_and_its_topic_must_be_offered() {
        let a = VillainAsk::One(scheme_ask());
        let parse = |raw: &str| parse_reply(&a, raw, no_bad_words);
        let ok = r#"{"line": "Behold, the Fraction Fog!", "mood": "scheming", "action": "brew",
                     "scheme": {"name": "Operation Fraction Fog", "topic": "Fractions", "gadget": "fog_machine"}}"#;
        let VillainReply::One(b) = parse(ok).unwrap() else {
            panic!()
        };
        let s = b.scheme.unwrap();
        assert_eq!(s.name, "Operation Fraction Fog");
        assert_eq!(s.topic, "fractions", "the offered spelling is kept");
        assert_eq!(s.gadget, Gadget::FogMachine);

        // An unknown gadget gets the topic's.
        let odd = ok.replace("fog_machine", "death_ray");
        let VillainReply::One(b) = parse(&odd).unwrap() else {
            panic!()
        };
        assert_eq!(b.scheme.unwrap().gadget, Gadget::FogMachine);

        // A topic that wasn't offered, a missing scheme, a long or blocked name: no.
        assert!(matches!(
            parse(&ok.replace("Fractions", "calculus")),
            Err(BeatError::BadScheme(_))
        ));
        assert!(matches!(
            parse(r#"{"line": "Behold!", "mood": "scheming"}"#),
            Err(BeatError::BadScheme(_))
        ));
        let long = ok.replace("Operation Fraction Fog", &"Very ".repeat(10));
        assert!(matches!(parse(&long), Err(BeatError::BadScheme(_))));
        let rude = ok.replace("Operation Fraction Fog", "Operation Stupid");
        assert!(matches!(parse(&rude), Err(BeatError::Blocked(_))));
    }

    #[test]
    fn the_prompt_carries_the_rules_the_cue_and_the_run() {
        let (system, user) = build_prompt(&VillainAsk::One(ask("flask_right")));
        assert!(system.contains("NEVER the player") && system.contains("JSON only"));
        assert!(system.contains("90 characters"));
        assert!(user.contains("got one right") && user.contains("120 m"));
        assert!(user.contains("Pinchy") && user.contains("fractions"));
        assert!(
            !user.contains("appear") && !user.contains("vanish"),
            "{user}"
        );
        assert!(user.contains("unveil"));

        let (_, user) = build_prompt(&VillainAsk::One(scheme_ask()));
        assert!(
            user.contains("\"scheme\"")
                && user.contains("division")
                && user.contains("fog_machine")
        );

        let (_, user) = build_prompt(&VillainAsk::Pair {
            right: ask("flask_right"),
            wrong: ask("flask_wrong"),
        });
        assert!(
            user.contains("\"right\"")
                && user.contains("\"wrong\"")
                && user.contains("about fractions")
        );
    }

    #[test]
    fn the_stand_in_models_replies_pass_the_checks() {
        for a in [
            VillainAsk::One(ask("idle")),
            VillainAsk::One(scheme_ask()),
            VillainAsk::Pair {
                right: ask("flask_right"),
                wrong: ask("flask_wrong"),
            },
        ] {
            let raw = fake_reply(&a);
            assert!(parse_reply(&a, &raw, no_bad_words).is_ok(), "{raw}");
        }
    }

    #[test]
    fn every_topic_has_a_fallback_scheme_that_fits_the_sign() {
        for t in [
            "addition",
            "subtraction",
            "multiplication",
            "division",
            "fractions",
            "place value",
            "perimeter and area",
            "word problems",
            "something new",
        ] {
            let s = fallback_scheme(t);
            assert!(s.name.chars().count() <= MAX_SCHEME_CHARS, "{}", s.name);
            assert_eq!(s.topic, t);
        }
        assert_ne!(
            fallback_scheme("addition").name,
            fallback_scheme("fractions").name
        );
    }
}
