//! A monster as data (FR21): a rig of parts that `game::monster_rig` draws
//! with the shared `gfx` vocabulary, plus a tier and a special that pick its
//! gameplay numbers from a fixed table. A spec is only ever art and a choice
//! between existing mechanics. It can't carry new rules or code, so a
//! hand-edited file and a model-written one are equally safe to load.
//!
//! [`parse`] is the only way in: it deserialises a JSON file and runs it
//! through [`validate`], which clamps every number into range, rejects
//! unknown enum values and unusable ids or names, and lightens any colour
//! that wouldn't read against the `INK` outlines. Plain Rust, no macroquad,
//! so it's unit-tested with hostile inputs below.

use serde::Deserialize;

/// Gameplay numbers for one kind of monster: the shipped roster's
/// (`game::monster::MonsterKind::info`) and a spec's ([`MonsterSpec::info`])
/// alike.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MonsterInfo {
    /// Correct answers needed to defeat it.
    pub hits: u32,
    /// Half-hearts a hit takes off the player.
    pub damage: u32,
    /// Its attack stuns the player for this long (seconds), if at all.
    pub stun_s: f32,
    /// Every wrong answer adds a half-heart to its damage.
    pub grows: bool,
    /// Its first hit steals the player's hat until it's beaten.
    pub steals_hat: bool,
    /// Only appears at milestones, never as a random encounter.
    pub boss: bool,
    /// The earliest distance up the road it can appear.
    pub min_m: u32,
    /// Added to the question difficulty when fighting it.
    pub difficulty_bias: u32,
    /// XP for each correct answer against it.
    pub xp_per_hit: u32,
}

/// RGB of `gfx::INK`, the outline colour every palette has to read against.
/// `game::monster_rig` has a test that keeps the two in step.
pub const INK_RGB: [f32; 3] = [0.13, 0.07, 0.12];
/// The least contrast ratio (WCAG-style) a palette colour needs against
/// `INK_RGB`. Darker colours are lightened until they reach it.
pub const MIN_CONTRAST: f32 = 3.5;

pub const NAME_MAX: usize = 16;
pub const BLURB_MAX: usize = 80;
pub const FACT_MAX: usize = 160;
pub const THEME_MAX: usize = 24;
pub const ID_MAX: usize = 32;
pub const MAX_PARTS: usize = 6;
pub const MAX_PART_COUNT: u32 = 4;
/// Body size limits, in sprite pixels: what fits the monster canvas
/// alongside its parts, idle lift and the grows special.
pub const BODY_W: (f32, f32) = (20.0, 76.0);
pub const BODY_H: (f32, f32) = (18.0, 64.0);
pub const EYE_SIZE: (f32, f32) = (2.0, 8.0);
pub const PART_SIZE: (f32, f32) = (0.5, 1.5);

/// A colour, 0..1 per channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub [f32; 3]);

impl Rgb {
    /// Parses `#rrggbb` (the `#` is optional).
    pub fn parse_hex(s: &str) -> Option<Rgb> {
        let h = s.trim().trim_start_matches('#');
        if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let ch = |i: usize| {
            u8::from_str_radix(&h[i..i + 2], 16)
                .ok()
                .map(|v| v as f32 / 255.0)
        };
        Some(Rgb([ch(0)?, ch(2)?, ch(4)?]))
    }

    /// Relative luminance (sRGB, as in WCAG).
    pub fn luminance(self) -> f32 {
        let lin = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let [r, g, b] = self.0;
        0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
    }

    pub fn contrast(self, other: Rgb) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// Mixes toward white in small steps until the colour stands out from
    /// the ink outlines by at least [`MIN_CONTRAST`].
    pub fn readable(self) -> Rgb {
        let ink = Rgb(INK_RGB);
        let mut c = self;
        for _ in 0..20 {
            if c.contrast(ink) >= MIN_CONTRAST {
                break;
            }
            c = Rgb(c.0.map(|v| v + (1.0 - v) * 0.12));
        }
        c
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyShape {
    Blob,
    Shell,
    Dome,
    Segmented,
    Wave,
    SpikyBall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EyeLook {
    Cute,
    Grumpy,
    Sleepy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartKind {
    Claw,
    Tentacle,
    Spike,
    Fin,
    Wing,
    Horn,
    Leg,
    Tail,
}

/// Which side of the body a part goes on. The monster faces left, toward
/// the player, so `Front` is its left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Front,
    Back,
    #[default]
    Both,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accessory {
    #[default]
    None,
    Crown,
    Mask,
    Helmet,
    BowTie,
    Leaf,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Idle {
    #[default]
    Bob,
    Scuttle,
    Float,
    Sway,
    Pulse,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Attack {
    #[default]
    Lunge,
    Zap,
    Spin,
    Slam,
    Spit,
}

/// One of the existing battle mechanics (FR10, FR12). A spec picks one; it
/// can't define a new one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Special {
    #[default]
    None,
    Stun,
    Grows,
    StealsHat,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Theme {
    pub subject: String,
    pub topic: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub shape: BodyShape,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub main: Rgb,
    pub accent: Rgb,
    pub belly: Rgb,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eyes {
    pub count: u32,
    pub size: f32,
    pub stalks: bool,
    pub look: EyeLook,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub kind: PartKind,
    pub count: u32,
    /// Relative size, 1.0 is normal.
    pub size: f32,
    pub side: Side,
}

/// A validated monster spec: every value is in range and every colour reads.
#[derive(Clone, Debug, PartialEq)]
pub struct MonsterSpec {
    pub id: String,
    pub name: String,
    pub blurb: String,
    /// A true real-world fact, for FR22's field guide.
    pub fact: String,
    /// Steers its questions once FR22 lands. Unused today.
    pub theme: Theme,
    pub body: Body,
    pub palette: Palette,
    pub eyes: Eyes,
    pub parts: Vec<Part>,
    pub accessory: Accessory,
    pub idle: Idle,
    pub attack: Attack,
    /// 1..=4: how tough it plays, from a Pinchling to a Dune Wyrm.
    pub tier: u32,
    pub special: Special,
}

// ---- The file format: loose on numbers, strict on enums -------------------

#[derive(Deserialize)]
struct RawSpec {
    id: String,
    name: String,
    #[serde(default)]
    blurb: String,
    #[serde(default)]
    fact: String,
    #[serde(default)]
    theme: Option<RawTheme>,
    body: RawBody,
    palette: RawPalette,
    eyes: RawEyes,
    #[serde(default)]
    parts: Vec<RawPart>,
    #[serde(default)]
    accessory: Accessory,
    #[serde(default)]
    idle: Idle,
    #[serde(default)]
    attack: Attack,
    tier: f64,
    #[serde(default)]
    special: Special,
}

#[derive(Deserialize)]
struct RawTheme {
    #[serde(default)]
    subject: String,
    #[serde(default)]
    topic: String,
}

#[derive(Deserialize)]
struct RawBody {
    shape: BodyShape,
    w: f64,
    h: f64,
}

#[derive(Deserialize)]
struct RawPalette {
    main: String,
    accent: String,
    belly: String,
}

#[derive(Deserialize)]
struct RawEyes {
    count: f64,
    size: f64,
    #[serde(default)]
    stalks: bool,
    #[serde(default = "default_look")]
    look: EyeLook,
}

fn default_look() -> EyeLook {
    EyeLook::Cute
}

#[derive(Deserialize)]
struct RawPart {
    kind: PartKind,
    #[serde(default = "one")]
    count: f64,
    #[serde(default = "one")]
    size: f64,
    #[serde(default)]
    side: Side,
}

fn one() -> f64 {
    1.0
}

/// Parses and validates one monster file. An `Err` says what was wrong,
/// for the toast and the log. The caller keeps the last good version.
pub fn parse(json: &str) -> Result<MonsterSpec, String> {
    let raw: RawSpec = serde_json::from_str(json).map_err(|e| e.to_string())?;
    validate(raw)
}

/// Clamps a number into `[lo, hi]`; NaN goes to `lo`.
fn clampf(v: f64, (lo, hi): (f32, f32)) -> f32 {
    if v.is_nan() {
        lo
    } else {
        (v as f32).clamp(lo, hi)
    }
}

fn clampu(v: f64, lo: u32, hi: u32) -> u32 {
    if v.is_nan() {
        lo
    } else {
        v.round().clamp(lo as f64, hi as f64) as u32
    }
}

/// Single-line text: control characters dropped, whitespace collapsed,
/// cut to `max` characters, at a word boundary where there is one.
fn clean_text(s: &str, max: usize) -> String {
    let words: Vec<&str> = s.split_whitespace().collect();
    let flat: Vec<char> = words
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    if flat.len() <= max {
        return flat.into_iter().collect();
    }
    let mut cut: String = flat[..max].iter().collect();
    if flat[max] != ' ' {
        if let Some(space) = cut.rfind(' ') {
            cut.truncate(space);
        }
    }
    cut.trim_end().to_string()
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= ID_MAX
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn color(name: &str, s: &str) -> Result<Rgb, String> {
    Rgb::parse_hex(s)
        .map(Rgb::readable)
        .ok_or_else(|| format!("palette.{name}: expected a #rrggbb colour, got {s:?}"))
}

fn validate(raw: RawSpec) -> Result<MonsterSpec, String> {
    if !valid_id(&raw.id) {
        return Err(format!(
            "id {:?}: use 1-{ID_MAX} lowercase letters, digits, '_' or '-'",
            raw.id
        ));
    }
    let name = clean_text(&raw.name, NAME_MAX);
    if name.is_empty() {
        return Err("name is empty".into());
    }
    let theme = raw.theme.map_or_else(Theme::default, |t| Theme {
        subject: clean_text(&t.subject, THEME_MAX),
        topic: clean_text(&t.topic, THEME_MAX),
    });
    Ok(MonsterSpec {
        id: raw.id,
        name,
        blurb: clean_text(&raw.blurb, BLURB_MAX),
        fact: clean_text(&raw.fact, FACT_MAX),
        theme,
        body: Body {
            shape: raw.body.shape,
            w: clampf(raw.body.w, BODY_W),
            h: clampf(raw.body.h, BODY_H),
        },
        palette: Palette {
            main: color("main", &raw.palette.main)?,
            accent: color("accent", &raw.palette.accent)?,
            belly: color("belly", &raw.palette.belly)?,
        },
        eyes: Eyes {
            count: clampu(raw.eyes.count, 1, 3),
            size: clampf(raw.eyes.size, EYE_SIZE),
            stalks: raw.eyes.stalks,
            look: raw.eyes.look,
        },
        parts: raw
            .parts
            .into_iter()
            .take(MAX_PARTS)
            .map(|p| Part {
                kind: p.kind,
                count: clampu(p.count, 1, MAX_PART_COUNT),
                size: clampf(p.size, PART_SIZE),
                side: p.side,
            })
            .collect(),
        accessory: raw.accessory,
        idle: raw.idle,
        attack: raw.attack,
        tier: clampu(raw.tier, 1, 4),
        special: raw.special,
    })
}

/// The toughest tier that can appear `meters` up the road: the tier table's
/// `min_m` bands (0, 50, 140 and 400 m). What the studio uses to decide which
/// tier to stock up on.
pub fn tier_for_meters(meters: u32) -> u32 {
    match meters {
        0..=49 => 1,
        50..=139 => 2,
        140..=399 => 3,
        _ => 4,
    }
}

impl MonsterSpec {
    /// The gameplay numbers, from the tier table alone: a spec can't set
    /// them. The tiers follow the shipped roster's bands (tier 1 plays like
    /// a Pinchling, tier 4 like a Dune Wyrm), and specs are never bosses.
    pub fn info(&self) -> MonsterInfo {
        let (hits, damage, min_m, difficulty_bias, xp_per_hit) = match self.tier {
            1 => (1, 1, 0, 0, 15),
            2 => (2, 1, 50, 0, 20),
            3 => (3, 2, 140, 1, 25),
            _ => (3, 2, 400, 2, 30),
        };
        MonsterInfo {
            hits,
            damage,
            stun_s: if self.special == Special::Stun {
                2.0
            } else {
                0.0
            },
            grows: self.special == Special::Grows,
            steals_hat: self.special == Special::StealsHat,
            boss: false,
            min_m,
            difficulty_bias,
            xp_per_hit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r##"{
        "id": "mossmunch", "name": "Mossmunch", "blurb": "Nibbles the road.",
        "body": {"shape": "blob", "w": 44, "h": 36},
        "palette": {"main": "#6fae4f", "accent": "#e8d25a", "belly": "#cfe8a0"},
        "eyes": {"count": 2, "size": 4.5, "look": "cute"},
        "parts": [{"kind": "leg", "count": 2}],
        "tier": 1
    }"##;

    fn with(field: &str, value: &str) -> String {
        let mut v: serde_json::Value = serde_json::from_str(GOOD).unwrap();
        let mut at = &mut v;
        let path: Vec<&str> = field.split('.').collect();
        for k in &path[..path.len() - 1] {
            at = &mut at[*k];
        }
        at[path[path.len() - 1]] = serde_json::from_str(value).unwrap();
        v.to_string()
    }

    #[test]
    fn a_good_spec_loads_with_defaults() {
        let s = parse(GOOD).unwrap();
        assert_eq!(s.name, "Mossmunch");
        assert_eq!(s.eyes.count, 2);
        assert!(!s.eyes.stalks);
        assert_eq!(s.accessory, Accessory::None);
        assert_eq!(s.idle, Idle::Bob);
        assert_eq!(s.attack, Attack::Lunge);
        assert_eq!(s.special, Special::None);
        assert_eq!(s.parts[0].side, Side::Both);
        assert_eq!(s.parts[0].size, 1.0);
    }

    #[test]
    fn huge_and_silly_numbers_are_clamped() {
        let s = parse(&with("body", r#"{"shape": "dome", "w": 1e9, "h": -5}"#)).unwrap();
        assert_eq!((s.body.w, s.body.h), (BODY_W.1, BODY_H.0));
        let s = parse(&with("eyes", r#"{"count": 40, "size": 0.01}"#)).unwrap();
        assert_eq!((s.eyes.count, s.eyes.size), (3, EYE_SIZE.0));
        let s = parse(&with("eyes", r#"{"count": -3, "size": 900}"#)).unwrap();
        assert_eq!((s.eyes.count, s.eyes.size), (1, EYE_SIZE.1));
        let s = parse(&with("tier", "99")).unwrap();
        assert_eq!(s.tier, 4);
        let s = parse(&with("tier", "0")).unwrap();
        assert_eq!(s.tier, 1);
        let parts = r#"[{"kind":"spike","count":50,"size":20}]"#;
        let s = parse(&with("parts", parts)).unwrap();
        assert_eq!(
            (s.parts[0].count, s.parts[0].size),
            (MAX_PART_COUNT, PART_SIZE.1)
        );
        let many = format!("[{}]", vec![r#"{"kind":"leg"}"#; 20].join(","));
        assert_eq!(parse(&with("parts", &many)).unwrap().parts.len(), MAX_PARTS);
    }

    #[test]
    fn unknown_enums_are_rejected() {
        for (field, value) in [
            ("body.shape", r#""dragon""#),
            ("special", r#""instant_kill""#),
            ("attack", r#""laser""#),
            ("idle", r#""teleport""#),
            ("accessory", r#""sword""#),
            ("eyes.look", r#""evil""#),
            ("parts", r#"[{"kind":"jetpack"}]"#),
        ] {
            assert!(parse(&with(field, value)).is_err(), "{field} = {value}");
        }
    }

    #[test]
    fn bad_ids_names_and_colours_are_rejected() {
        let long = format!("{:?}", "x".repeat(40));
        for id in [
            r#""""#,
            r#""../../etc/passwd""#,
            r#""Moss Munch""#,
            long.as_str(),
        ] {
            assert!(parse(&with("id", id)).is_err(), "id {id}");
        }
        assert!(parse(&with("name", r#""   ""#)).is_err());
        assert!(parse(&with("palette.main", r#""green""#)).is_err());
        assert!(parse(&with("palette.belly", r##""#12345""##)).is_err());
        assert!(parse("not json").is_err());
        assert!(parse("{}").is_err());
    }

    #[test]
    fn long_or_messy_text_is_cut_and_cleaned() {
        let s = parse(&with("name", r#""The\nIncredibly  Long Monster Name""#)).unwrap();
        assert!(s.name.chars().count() <= NAME_MAX);
        assert!(!s.name.contains('\n'));
        assert_eq!(s.name, "The Incredibly");
        let s = parse(&with("blurb", &format!("{:?}", "word ".repeat(100)))).unwrap();
        assert!(s.blurb.chars().count() <= BLURB_MAX);
    }

    #[test]
    fn invisible_colours_are_lightened_until_they_read() {
        for hex in ["#000000", "#221122", "#200d1f", "#0a0a30"] {
            let s = parse(&with("palette.main", &format!("{hex:?}"))).unwrap();
            assert!(
                s.palette.main.contrast(Rgb(INK_RGB)) >= MIN_CONTRAST,
                "{hex} -> {:?}",
                s.palette.main
            );
        }
        // A colour that already reads is left alone.
        let s = parse(GOOD).unwrap();
        assert_eq!(s.palette.main, Rgb::parse_hex("#6fae4f").unwrap());
    }

    #[test]
    fn the_tier_for_a_distance_matches_the_tier_table() {
        for tier in 1..=4 {
            let spec = parse(&with("tier", &tier.to_string())).unwrap();
            let min_m = spec.info().min_m;
            assert_eq!(tier_for_meters(min_m), tier, "from {min_m} m");
            if min_m > 0 {
                assert_eq!(tier_for_meters(min_m - 1), tier - 1);
            }
        }
    }

    #[test]
    fn stats_come_from_the_tier_table_only() {
        // Extra fields that try to set stats are ignored.
        let sneaky =
            with("hits", "0").replace("\"tier\":1", "\"tier\":1,\"damage\":99,\"boss\":true");
        let s = parse(&sneaky).unwrap();
        let info = s.info();
        assert_eq!((info.hits, info.damage, info.boss), (1, 1, false));
        let mut last = (0, 0);
        for tier in 1..=4 {
            let info = parse(&with("tier", &tier.to_string())).unwrap().info();
            assert!(info.hits >= 1 && info.damage >= 1 && !info.boss);
            assert!(
                (info.hits, info.xp_per_hit) >= last,
                "tier {tier} is tougher"
            );
            last = (info.hits, info.xp_per_hit);
        }
        let stun = parse(&with("special", r#""stun""#)).unwrap().info();
        assert!(stun.stun_s > 0.0 && !stun.grows && !stun.steals_hat);
        assert!(parse(&with("special", r#""grows""#)).unwrap().info().grows);
        assert!(
            parse(&with("special", r#""steals_hat""#))
                .unwrap()
                .info()
                .steals_hat
        );
    }
}
