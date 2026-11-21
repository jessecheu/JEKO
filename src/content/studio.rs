//! The studio (FR21, FR22): one long-lived background thread that designs new
//! monsters, writes lesson packs and Professor Pebble's tips with the model
//! while the player walks, and writes each one into the content dir as a JSON
//! file. The watcher then loads it like any other file, so generated content
//! and hand-written content take the same path into the game.
//!
//! The studio is the lowest priority for the model (FR32): it keeps at most
//! one request in flight, never starts one while the game says a question is
//! loading or being prefetched (`set_live_busy`), and stays under a
//! per-minute cap and a session token budget. It never blocks a frame: the
//! game only talks to it through `enqueue`, `status` and `set_live_busy`,
//! none of which wait on the network.
//!
//! The model writes data and the game keeps the rules: everything it returns
//! goes through [`finish_monster`], which forces the requested tier, limits
//! the special to the ones generated monsters may use, vets the text, and
//! hands the result to `monster_spec::parse` before a byte is written.

use super::monster_spec::{self, ID_MAX};
use super::{lessons, prompts, tips};
use crate::llm::cerebras_client::CerebrasClient;
use crate::llm::task_gen::{self, extract_json_object};
use crate::llm::usage::RequestKind;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Generated monsters are written as `gen-<name>`, so the game (and a
/// `.gitignore`) can tell them from the hand-written ones.
pub const GENERATED_PREFIX: &str = "gen-";

/// True for a spec the studio wrote.
pub fn is_generated(id: &str) -> bool {
    id.starts_with(GENERATED_PREFIX)
}

/// Studio requests per minute unless `JEKO_STUDIO_RPM` says otherwise.
pub const DEFAULT_RPM: usize = 6;
/// Tokens the studio may spend in a session unless `JEKO_STUDIO_TOKENS` says
/// otherwise. A monster costs about 1,000.
pub const DEFAULT_TOKEN_BUDGET: u64 = 60_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StudioConfig {
    /// `JEKO_STUDIO=off` disables the studio.
    pub enabled: bool,
    pub rpm: usize,
    pub token_budget: u64,
}

impl StudioConfig {
    pub fn from_env() -> Self {
        let var = |k| std::env::var(k).ok();
        Self::parse(
            var("JEKO_STUDIO").as_deref(),
            var("JEKO_STUDIO_RPM").as_deref(),
            var("JEKO_STUDIO_TOKENS").as_deref(),
        )
    }

    fn parse(switch: Option<&str>, rpm: Option<&str>, tokens: Option<&str>) -> Self {
        Self {
            enabled: !matches!(
                switch.map(|s| s.trim().to_ascii_lowercase()).as_deref(),
                Some("off" | "0" | "false" | "no")
            ),
            rpm: rpm
                .and_then(|r| r.trim().parse().ok())
                .unwrap_or(DEFAULT_RPM),
            token_budget: tokens
                .and_then(|t| t.trim().parse().ok())
                .unwrap_or(DEFAULT_TOKEN_BUDGET),
        }
    }
}

/// Design one monster.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonsterJob {
    /// 1..=4, from `MonsterSpec::tier`.
    pub tier: u32,
    /// The maths topic its questions will lean on.
    pub topic: String,
    /// Names already taken, so the model doesn't repeat them.
    pub avoid: Vec<String>,
}

/// Write a pack of practice problems (FR22).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LessonJob {
    pub topic: String,
    /// 1..=5.
    pub level: u32,
    /// How many problems to ask for.
    pub n: usize,
    /// A problem the player just missed: the pack should be similar practice.
    pub like: Option<String>,
}

/// Write Professor Pebble's tips on a topic the player keeps missing (FR22).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TipsJob {
    pub npc: String,
    pub topic: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Job {
    Monster(MonsterJob),
    Lessons(LessonJob),
    Tips(TipsJob),
}

impl Job {
    fn kind(&self) -> RequestKind {
        match self {
            Job::Monster(_) => RequestKind::Monster,
            Job::Lessons(_) | Job::Tips(_) => RequestKind::Lesson,
        }
    }
}

/// What the studio has done, for the token panel and the dev summary.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StudioStatus {
    /// Jobs waiting, not counting the one being worked on.
    pub queued: usize,
    /// A job is being worked on (waiting for its turn, or in flight).
    pub working: bool,
    /// Files written to the content dir (monsters, packs and tips).
    pub made: u32,
    /// Of `made`, the lesson packs.
    pub packs: u32,
    /// Of `made`, the tips files.
    pub tips: u32,
    /// Replies the vetting turned down (bad JSON, off-limits words, invalid spec).
    pub rejected: u32,
    /// Calls that failed outright.
    pub failed: u32,
    /// The last thing that went wrong.
    pub last_error: Option<String>,
    /// The session token budget is spent: the studio has stopped.
    pub out_of_budget: bool,
}

struct Shared {
    queue: Mutex<VecDeque<Job>>,
    wake: Condvar,
    status: Mutex<StudioStatus>,
    live_busy: AtomicBool,
    stop: AtomicBool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The handle the game holds. Dropping it stops the thread.
pub struct Studio {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Studio {
    /// Starts the worker on `content_dir`. `None` when the studio is switched
    /// off or there's nothing to ask: no API key (and no stand-in model).
    pub fn spawn(client: CerebrasClient, content_dir: PathBuf, cfg: StudioConfig) -> Option<Self> {
        if !cfg.enabled || !client.has_api_key() {
            return None;
        }
        let shared = Arc::new(Shared {
            queue: Mutex::default(),
            wake: Condvar::new(),
            status: Mutex::default(),
            live_busy: AtomicBool::new(false),
            stop: AtomicBool::new(false),
        });
        let worker = shared.clone();
        let thread = std::thread::Builder::new()
            .name("studio".into())
            .spawn(move || work(&worker, &client, &content_dir, cfg))
            .expect("spawn the studio thread");
        Some(Self {
            shared,
            thread: Some(thread),
        })
    }

    pub fn enqueue(&self, job: Job) {
        lock(&self.shared.queue).push_back(job);
        self.shared.wake.notify_one();
    }

    /// Jobs queued plus the one being worked on.
    pub fn pending(&self) -> usize {
        let s = self.status();
        s.queued + usize::from(s.working)
    }

    pub fn status(&self) -> StudioStatus {
        let queued = lock(&self.shared.queue).len();
        StudioStatus {
            queued,
            ..lock(&self.shared.status).clone()
        }
    }

    /// Tells the studio that something that outranks it is using the model
    /// (a question loading, a prefetch in flight, a pause). It starts no
    /// request while this is true.
    pub fn set_live_busy(&self, busy: bool) {
        self.shared.live_busy.store(busy, Ordering::Relaxed);
    }
}

impl Drop for Studio {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.wake.notify_all();
        // Not joined: a request in flight can take seconds, and the thread
        // ends by itself as soon as it's back.
        drop(self.thread.take());
    }
}

const GATE_POLL: Duration = Duration::from_millis(100);

fn work(shared: &Shared, client: &CerebrasClient, dir: &Path, cfg: StudioConfig) {
    let mut sent: VecDeque<Instant> = VecDeque::new();
    let mut seq: u64 = 0;
    'jobs: loop {
        // Wait for a job.
        let job = {
            let mut q = lock(&shared.queue);
            loop {
                if shared.stop.load(Ordering::Relaxed) {
                    return;
                }
                if let Some(j) = q.pop_front() {
                    lock(&shared.status).working = true;
                    break j;
                }
                q = shared
                    .wake
                    .wait_timeout(q, Duration::from_millis(500))
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
        };
        // Wait for its turn: the lowest lane, under the minute cap and budget.
        loop {
            if shared.stop.load(Ordering::Relaxed) {
                return;
            }
            let spent = {
                let stats = client.token_stats();
                stats.kind(RequestKind::Monster).tokens + stats.kind(RequestKind::Lesson).tokens
            };
            if spent >= cfg.token_budget {
                lock(&shared.queue).clear();
                let mut st = lock(&shared.status);
                st.out_of_budget = true;
                st.working = false;
                continue 'jobs;
            }
            let now = Instant::now();
            while sent
                .front()
                .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(60))
            {
                sent.pop_front();
            }
            if !shared.live_busy.load(Ordering::Relaxed) && sent.len() < cfg.rpm {
                break;
            }
            std::thread::sleep(GATE_POLL);
        }
        sent.push_back(Instant::now());
        seq += 1;
        let result = run(&job, client, dir, seq);
        let mut st = lock(&shared.status);
        st.working = false;
        match result {
            Ok(made) => {
                st.made += 1;
                match made {
                    Made::Monster(_) => {}
                    Made::Pack { .. } => st.packs += 1,
                    Made::Tips => st.tips += 1,
                }
            }
            Err(Failure::Rejected(e)) => {
                st.rejected += 1;
                st.last_error = Some(e);
            }
            Err(Failure::Call(e)) => {
                st.failed += 1;
                st.last_error = Some(e);
            }
        }
    }
}

enum Failure {
    /// The model answered, and the answer wasn't usable.
    Rejected(String),
    /// The call itself failed.
    Call(String),
}

/// What a finished job produced.
enum Made {
    /// A monster, by id.
    Monster(String),
    /// A lesson pack: problems kept, and problems turned down.
    #[allow(dead_code)]
    Pack {
        kept: usize,
        dropped: usize,
    },
    Tips,
}

/// Designs one monster right now and writes it to `dir/monsters/`, on the
/// caller's thread: the same code path as the studio's worker, for the
/// offline batch tool (`gen_content --monsters`). Returns the new id.
pub fn generate_blocking(
    client: &CerebrasClient,
    dir: &Path,
    job: &MonsterJob,
) -> Result<String, String> {
    match run(&Job::Monster(job.clone()), client, dir, seed(0)) {
        Ok(Made::Monster(id)) => Ok(id),
        Ok(_) => Err("unexpected result".into()),
        Err(Failure::Rejected(e)) => Err(format!("turned down: {e}")),
        Err(Failure::Call(e)) => Err(format!("the call failed: {e}")),
    }
}

/// Asks for (or, with the stand-in model, makes up) the reply to a job.
fn ask(
    job: &Job,
    client: &CerebrasClient,
    prompt: impl FnOnce() -> (String, String),
    tokens: (u64, u64),
    fake: impl FnOnce() -> String,
) -> Result<String, Failure> {
    match client.fake_delay() {
        // The stand-in model (`JEKO_DEV=fakemodel`): made-up content after a pause.
        Some(delay) => {
            std::thread::sleep(delay);
            client.record_fake(job.kind(), tokens.0, tokens.1, delay);
            Ok(fake())
        }
        None => {
            let (system, user) = prompt();
            client
                .request_tagged_blocking(job.kind(), &system, &user)
                .map_err(|e| Failure::Call(e.to_string()))
        }
    }
}

fn run(job: &Job, client: &CerebrasClient, dir: &Path, seq: u64) -> Result<Made, Failure> {
    match job {
        Job::Monster(m) => {
            let raw = ask(
                job,
                client,
                || prompts::monster_prompt(m.tier, &m.topic, &m.avoid),
                (700, 300),
                || fake_monster_json(seed(seq), m.tier, &m.topic),
            )?;
            finish_monster(&raw, dir, m.tier, &m.topic)
                .map(Made::Monster)
                .map_err(Failure::Rejected)
        }
        Job::Lessons(l) => {
            let raw = ask(
                job,
                client,
                || task_gen::build_pack_prompt(&l.topic, l.level, l.n, l.like.as_deref()),
                (300, 900),
                || lessons::fake_pack_json(l.n),
            )?;
            lessons::finish_pack(&raw, dir, &l.topic, l.level, l.n)
                .map(|(_, pack)| Made::Pack {
                    kept: pack.tasks.len(),
                    dropped: pack.dropped,
                })
                .map_err(Failure::Rejected)
        }
        Job::Tips(t) => {
            let raw = ask(
                job,
                client,
                || tips::tips_prompt(&t.topic),
                (150, 120),
                || fake_tips_json(&t.topic),
            )?;
            tips::finish(&raw, dir, &t.npc, &t.topic)
                .map(|_| Made::Tips)
                .map_err(Failure::Rejected)
        }
    }
}

/// Made-up tips for the stand-in model.
fn fake_tips_json(topic: &str) -> String {
    json!([
        format!("Take {topic} one small step at a time."),
        "Say the problem out loud, then try it with something you can count.",
        "Check your answer by working it backwards."
    ])
    .to_string()
}

fn seed(seq: u64) -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos() as u64);
    seq.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ nanos
}

/// Turns the model's reply into a monster file in `dir/monsters/`, or says
/// why not. Returns the new monster's id. The only thing that writes a
/// generated spec, and everything it writes has passed `monster_spec::parse`:
///
/// - the tier is the one asked for, whatever the reply says;
/// - the special is `none` or `grows`; anything else becomes `none`;
/// - the name, blurb and fact are vetted ([`vet_text`]);
/// - the id is made here, from the name, and is unique in the dir.
pub fn finish_monster(raw: &str, dir: &Path, tier: u32, topic: &str) -> Result<String, String> {
    let slice = extract_json_object(raw).ok_or("no JSON object in the reply")?;
    let mut v: Value = serde_json::from_str(slice).map_err(|e| format!("bad JSON: {e}"))?;
    let obj = v.as_object_mut().ok_or("the reply isn't a JSON object")?;

    obj.insert("tier".into(), json!(tier));
    let special_ok = obj
        .get("special")
        .and_then(Value::as_str)
        .is_some_and(|s| prompts::GENERATED_SPECIALS.contains(&s));
    if !special_ok {
        obj.insert("special".into(), json!("none"));
    }
    if obj.get("theme").is_none_or(|t| !t.is_object()) {
        obj.insert("theme".into(), json!({"subject": "math", "topic": topic}));
    }
    let name = obj
        .get("name")
        .and_then(Value::as_str)
        .ok_or("the reply has no name")?
        .to_string();

    let monsters = dir.join("monsters");
    let id = unique_id(&monsters, &name);
    obj.insert("id".into(), json!(id));
    let spec = monster_spec::parse(&v.to_string())?;
    for (field, text) in [
        ("name", &spec.name),
        ("blurb", &spec.blurb),
        ("fact", &spec.fact),
    ] {
        vet_text(text).map_err(|w| format!("{field} has an off-limits word: {w}"))?;
    }
    vet_text(&spec.theme.topic).map_err(|w| format!("theme has an off-limits word: {w}"))?;

    std::fs::create_dir_all(&monsters).map_err(|e| e.to_string())?;
    let pretty = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    // Write beside the file and rename it into place: the watcher polls the
    // dir, so it must never see a half-written spec. `.tmp` isn't `.json`, so
    // it ignores the temp file.
    let tmp = monsters.join(format!("{id}.json.tmp"));
    std::fs::write(&tmp, pretty).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, monsters.join(format!("{id}.json"))).map_err(|e| e.to_string())?;
    Ok(id)
}

/// `gen-<slug of name>`, with `-2`, `-3`… if the file already exists.
fn unique_id(monsters: &Path, name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "monster" } else { slug };
    let room = ID_MAX - GENERATED_PREFIX.len() - 4;
    let slug: String = slug.chars().take(room).collect();
    let slug = slug.trim_end_matches('-');
    let base = format!("{GENERATED_PREFIX}{slug}");
    let mut id = base.clone();
    let mut n = 2;
    while monsters.join(format!("{id}.json")).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// Words a monster's text may not use: other people's characters, because the
/// prompt asks for original creatures, and anything that isn't for a children's
/// game. Matched on whole words, ignoring case. A blocklist is a floor, not a
/// guarantee: the kid-safe prompt does the main work, and the owner's call on
/// whether a person reviews generated monsters is still open (FR21).
const BLOCKED: &[&str] = &[
    // Characters and brands.
    "pokemon",
    "pikachu",
    "digimon",
    "mario",
    "luigi",
    "bowser",
    "goomba",
    "sonic",
    "zelda",
    "kirby",
    "mickey",
    "disney",
    "pixar",
    "shrek",
    "minecraft",
    "creeper",
    "nintendo",
    "marvel",
    "pacman",
    "yoshi",
    "totoro",
    "godzilla",
    // Not for a children's game.
    "kill",
    "kills",
    "killed",
    "murder",
    "blood",
    "bloody",
    "gore",
    "corpse",
    "dead",
    "death",
    "suicide",
    "gun",
    "guns",
    "rifle",
    "bomb",
    "knife",
    "sword",
    "weapon",
    "drug",
    "drugs",
    "beer",
    "wine",
    "alcohol",
    "sex",
    "sexy",
    "naked",
    "damn",
    "hell",
    "stupid",
    "idiot",
];

/// `Err(word)` if `text` uses a blocked word.
pub fn vet_text(text: &str) -> Result<(), String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .find(|w| BLOCKED.contains(&w.as_str()))
        .map_or(Ok(()), Err)
}

/// A made-up creature for the stand-in model, so the studio can be run end to
/// end with no key. Varied by `seed`, and always a spec that passes
/// `finish_monster`.
pub fn fake_monster_json(seed: u64, tier: u32, topic: &str) -> String {
    let mut r = Rng(seed);
    const HEADS: &[&str] = &[
        "Thistle", "Glim", "Bramble", "Fern", "Mirk", "Puddle", "Wisp", "Burr", "Lantern", "Moss",
    ];
    const TAILS: &[&str] = &[
        "snout", "wing", "toad", "kin", "mouse", "back", "light", "nip", "hopper", "fluff",
    ];
    let name = format!("{}{}", r.pick(HEADS), r.pick(TAILS));
    let mut colour = || {
        let n = r.next();
        format!(
            "#{:02x}{:02x}{:02x}",
            90 + n % 140,
            90 + (n / 7) % 140,
            90 + (n / 13) % 140
        )
    };
    let (c1, c2, c3) = (colour(), colour(), colour());
    json!({
        "name": name,
        "blurb": "A made-up forest creature from the stand-in model.",
        "fact": "Owls can turn their heads about 270 degrees to look around.",
        "theme": {"subject": "math", "topic": topic},
        "body": {"shape": r.pick(prompts::SHAPES), "w": 30 + r.next() % 30, "h": 26 + r.next() % 24},
        "palette": {"main": c1, "accent": c2, "belly": c3},
        "eyes": {"count": 1 + r.next() % 3, "size": 3 + r.next() % 4, "stalks": r.next().is_multiple_of(2), "look": r.pick(prompts::LOOKS)},
        "parts": [
            {"kind": r.pick(prompts::PARTS), "count": 1 + r.next() % 3, "size": 1.0, "side": r.pick(prompts::SIDES)},
            {"kind": r.pick(prompts::PARTS), "count": 1 + r.next() % 3, "size": 0.8, "side": "both"},
        ],
        "accessory": r.pick(prompts::ACCESSORIES),
        "idle": r.pick(prompts::IDLES),
        "attack": r.pick(prompts::ATTACKS),
        "tier": tier,
        "special": r.pick(prompts::GENERATED_SPECIALS),
    })
    .to_string()
}

/// splitmix64, for the stand-in model's choices.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn pick(&mut self, xs: &[&str]) -> String {
        xs[(self.next() % xs.len() as u64) as usize].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::content::monster_spec::{parse, Special};

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("jeko-studio-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn reply(name: &str, extra: &str) -> String {
        format!(
            r##"Here you go: {{"id":"whatever","name":"{name}","blurb":"Hops about.",
            "fact":"Frogs breathe partly through their skin.",
            "body":{{"shape":"dome","w":40,"h":30}},
            "palette":{{"main":"#88cc66","accent":"#ffcc44","belly":"#ddeebb"}},
            "eyes":{{"count":2,"size":4}},"tier":4 {extra}}}"##
        )
    }

    #[test]
    fn a_good_reply_becomes_a_loadable_file_with_the_game_setting_the_rules() {
        let d = temp_dir("good");
        let id = finish_monster(
            &reply("Puddlehop", r#","special":"stun""#),
            &d,
            2,
            "fractions",
        )
        .unwrap();
        assert_eq!(id, "gen-puddlehop");
        let text = std::fs::read_to_string(d.join("monsters/gen-puddlehop.json")).unwrap();
        let spec = parse(&text).unwrap();
        assert_eq!(
            spec.id, "gen-puddlehop",
            "the id is made here, not by the model"
        );
        assert_eq!(spec.tier, 2, "the tier is the one asked for");
        assert_eq!(spec.special, Special::None, "stun is for hand-written ones");
        assert_eq!(spec.theme.topic, "fractions");
        assert!(is_generated(&spec.id));
        assert!(!d.join("monsters/gen-puddlehop.json.tmp").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn grows_is_allowed_and_names_never_collide() {
        let d = temp_dir("dupes");
        let a = finish_monster(
            &reply("Fern Imp!", r#","special":"grows""#),
            &d,
            1,
            "addition",
        )
        .unwrap();
        let b = finish_monster(&reply("Fern Imp!", ""), &d, 1, "addition").unwrap();
        let c = finish_monster(&reply("  ", ""), &d, 1, "addition");
        assert_eq!((a.as_str(), b.as_str()), ("gen-fern-imp", "gen-fern-imp-2"));
        assert!(c.is_err(), "an empty name is rejected by the parser");
        let spec =
            parse(&std::fs::read_to_string(d.join("monsters/gen-fern-imp.json")).unwrap()).unwrap();
        assert_eq!(spec.special, Special::Grows);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn bad_replies_are_rejected_and_write_nothing() {
        let d = temp_dir("bad");
        for raw in [
            "I'm sorry, I can't do that.",
            "{ not json }",
            "[1, 2, 3]",
            r#"{"name": "NoBody"}"#,
            &reply("Pikachu Junior", ""),
            &reply("Hell Beast", ""),
            &reply("Okay", r#","blurb":"It will kill you.""#),
        ] {
            assert!(finish_monster(raw, &d, 1, "addition").is_err(), "{raw}");
        }
        let files = std::fs::read_dir(d.join("monsters"))
            .map(|r| r.count())
            .unwrap_or(0);
        assert_eq!(files, 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn hostile_numbers_in_a_reply_are_clamped_like_any_spec() {
        let d = temp_dir("hostile");
        let raw = reply("Chonk", "").replace(r#""w":40,"h":30"#, r#""w":99999,"h":-5"#);
        let id = finish_monster(&raw, &d, 3, "division").unwrap();
        let spec = parse(&std::fs::read_to_string(d.join(format!("monsters/{id}.json"))).unwrap())
            .unwrap();
        assert!(spec.body.w <= monster_spec::BODY_W.1 && spec.body.h >= monster_spec::BODY_H.0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn ids_fit_the_limit_and_the_alphabet() {
        let d = temp_dir("ids");
        let id = unique_id(&d, "Ünïcödé Ünderscore & A Ridiculously Long Creature Name");
        assert!(id.len() <= ID_MAX, "{id}");
        assert!(id.starts_with("gen-") && !id.ends_with('-'));
        assert!(id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
        assert_eq!(unique_id(&d, "!!!"), "gen-monster");
    }

    #[test]
    fn vetting_matches_whole_words_only() {
        assert!(vet_text("A kind little frog").is_ok());
        assert!(vet_text("Shellfish and Hellebore").is_ok(), "not 'hell'");
        assert!(vet_text("Bloodroot").is_ok(), "not 'blood'");
        assert!(vet_text("It's Mario!").is_err());
        assert!(vet_text("sneaky, DEATH-defying").is_err());
    }

    #[test]
    fn the_stand_in_models_creatures_always_pass() {
        let d = temp_dir("fake");
        for seed in 0..300 {
            let raw = fake_monster_json(seed, 1 + (seed % 4) as u32, "addition");
            finish_monster(&raw, &d, 1 + (seed % 4) as u32, "addition")
                .unwrap_or_else(|e| panic!("seed {seed}: {e}\n{raw}"));
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn env_switches_parse() {
        let d = StudioConfig::parse(None, None, None);
        assert_eq!(
            d,
            StudioConfig {
                enabled: true,
                rpm: DEFAULT_RPM,
                token_budget: DEFAULT_TOKEN_BUDGET
            }
        );
        assert!(!StudioConfig::parse(Some("off"), None, None).enabled);
        assert_eq!(StudioConfig::parse(None, Some("2"), Some("500")).rpm, 2);
        assert_eq!(
            StudioConfig::parse(None, Some("2"), Some("500")).token_budget,
            500
        );
        assert_eq!(
            StudioConfig::parse(None, Some("x"), Some("y")).rpm,
            DEFAULT_RPM
        );
    }

    fn fake_client() -> CerebrasClient {
        CerebrasClient::new(&Config {
            api_key: None,
            model: "m".into(),
            base_url: "http://127.0.0.1:9".into(),
        })
        .with_fake_model(Duration::from_millis(5))
    }

    fn job(tier: u32) -> Job {
        Job::Monster(MonsterJob {
            tier,
            topic: "addition".into(),
            avoid: vec![],
        })
    }

    fn wait_for(studio: &Studio, done: impl Fn(&StudioStatus) -> bool) -> StudioStatus {
        for _ in 0..200 {
            let s = studio.status();
            if done(&s) {
                return s;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("the studio didn't get there: {:?}", studio.status());
    }

    #[test]
    fn the_worker_writes_one_file_per_job_and_waits_for_live_questions() {
        let d = temp_dir("worker");
        let cfg = StudioConfig {
            enabled: true,
            rpm: 60,
            token_budget: u64::MAX,
        };
        let studio = Studio::spawn(fake_client(), d.clone(), cfg)
            .expect("the stand-in model counts as a key");

        // A live question is loading: the studio holds its job.
        studio.set_live_busy(true);
        studio.enqueue(job(1));
        std::thread::sleep(Duration::from_millis(400));
        let s = studio.status();
        assert_eq!((s.made, s.working), (0, true));
        assert_eq!(studio.pending(), 1);

        // It's clear again: the job runs, then the next.
        studio.set_live_busy(false);
        studio.enqueue(job(2));
        let s = wait_for(&studio, |s| s.made == 2);
        assert_eq!(
            (s.rejected, s.failed, s.working, s.queued),
            (0, 0, false, 0)
        );
        let n = std::fs::read_dir(d.join("monsters")).unwrap().count();
        assert_eq!(n, 2);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_minute_cap_holds_jobs_back() {
        let d = temp_dir("rpm");
        let cfg = StudioConfig {
            enabled: true,
            rpm: 1,
            token_budget: u64::MAX,
        };
        let studio = Studio::spawn(fake_client(), d.clone(), cfg).unwrap();
        studio.enqueue(job(1));
        studio.enqueue(job(1));
        wait_for(&studio, |s| s.made == 1);
        std::thread::sleep(Duration::from_millis(400));
        let s = studio.status();
        assert_eq!(s.made, 1, "the second waits for the minute to pass");
        assert_eq!(studio.pending(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_spent_token_budget_stops_the_studio() {
        let d = temp_dir("budget");
        let client = fake_client();
        let cfg = StudioConfig {
            enabled: true,
            rpm: 60,
            token_budget: 100,
        };
        let studio = Studio::spawn(client, d.clone(), cfg).unwrap();
        // The stand-in model books 1,000 tokens per monster, so one job spends it.
        studio.enqueue(job(1));
        studio.enqueue(job(1));
        studio.enqueue(job(1));
        let s = wait_for(&studio, |s| s.out_of_budget);
        assert_eq!((s.made, s.queued), (1, 0), "{s:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn no_key_or_switched_off_means_no_studio() {
        let cfg = StudioConfig::parse(None, None, None);
        let offline = CerebrasClient::new(&Config {
            api_key: None,
            model: "m".into(),
            base_url: "http://127.0.0.1:9".into(),
        });
        assert!(Studio::spawn(offline, temp_dir("none"), cfg).is_none());
        let off = StudioConfig {
            enabled: false,
            ..cfg
        };
        assert!(Studio::spawn(fake_client(), temp_dir("off"), off).is_none());
    }
}
