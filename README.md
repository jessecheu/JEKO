# JEKO — Math Quest

A 2.5D exploration game set in a dark forest at night: you walk 1,500 m up a
winding cobbled road past a black lake, through fields and a village, to the
gate of a mad scientist's castle. Glowing potion flasks and the monsters on the
road ask math problems that Cerebras-hosted Kimi K3.1 writes live, scaled to
the player's level; the game asks for them before the player arrives, so most
are ready on the spot. The same model writes the mad scientist's taunts and,
in the background, new monsters, lesson packs and tips. Built for a hackathon
demo of Cerebras' low-latency inference: generation should be fast enough that
it never breaks the player's movement flow. With no API key the game still
plays fully offline, from question and line banks compiled into the binary.

How the code fits together is in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md);
the backlog is [FeatureRequestToDo.md](FeatureRequestToDo.md).

## Stack

- **macroquad** — window, input, and world rendering (the forest, its props,
  and the player are all flat-colored primitives for now; swap in real textures via
  `load_texture`/`draw_texture_ex` when art exists).
- **egui** via **egui-macroquad** — HUD overlay (progress bar, streak,
  upgrades, task panel). Chosen over imgui-rs because `egui-macroquad` is the
  maintained, first-class UI integration for macroquad; imgui's macroquad
  bindings are effectively unmaintained.
- **reqwest (blocking) + a background thread** — macroquad's `next_frame().await`
  only yields once per frame; it is not a real async executor. Network calls
  to Cerebras run on a plain `std::thread`, and the game loop polls an
  `mpsc::Receiver` each frame instead of awaiting the request in-line. See
  `CerebrasClient::request_task_async` in `src/llm/cerebras_client.rs`.
- **serde / serde_json** — the model's JSON replies, the compiled-in banks
  (questions, NPC and villain lines) and the content dir's monster, lesson and
  tip files.

## Project layout

```
Makefile                 every build/run/test target (`make help`); wraps `nix develop --command`
flake.nix                the dev shell (and an opt-in `openclaw` shell); `nix build`
secretspec.toml          declares CEREBRAS_API_KEY / _MODEL / _BASE_URL for secretspec
.env.example             copy to .env and fill in the key
AGENTS.md, CLAUDE.md     instructions for coding agents (CLAUDE.md imports AGENTS.md)
FeatureRequestToDo.md    the backlog, and where each feature stands
docs/ARCHITECTURE.md     how the code fits together, as built, and the rules a change must keep
src/
  lib.rs                 shared library (so the game and the two offline tools in src/bin/ can all use it)
  main.rs                game entry point / macroquad loop
  config.rs               reads CEREBRAS_API_KEY / CEREBRAS_MODEL / CEREBRAS_BASE_URL from .env; instance_name() labels the checkout
  content/
    mod.rs                content events, the roster of loaded spec monsters, where the content dir is (JEKO_CONTENT_DIR)
    monster_spec.rs       a monster as JSON data: parse + validate (clamped, unit-tested), stats from a tier table
    watcher.rs            background thread that rescans content/monsters/*.json and hot-reloads edits
    studio.rs             background thread that has the model design new monsters, write lesson packs and tips, into the content dir
    lessons.rs            lesson packs: practice problems whose answers are verified before they load
    tips.rs               Professor Pebble's tips on topics the player keeps missing
    prompts.rs            what the studio asks the model for
    vet.rs                the word blocklist for NPC lines and generated text
  llm/
    mod.rs                MathTask (the one question type) and the compiled-in fallback bank
    activity.rs           what the model is writing now (jobs for the creation bar; a read-only mirror)
    buffer.rs             ready-question buffer, the pure look-ahead planner, and the wait readout
    expr.rs               a tiny arithmetic evaluator that checks a generated problem's answer against its own sum
    cerebras_client.rs    OpenAI-compatible chat-completions call to Cerebras, async wrapper
    task_gen.rs           prompt templates + JSON extraction helpers
    villain_gen.rs        the mad scientist's prompt and the pure checks on a live line or scheme (moods, actions, gadgets) (FR23)
    usage.rs              per-call tokens, speed + timing, calls in flight, windowed usage rate (feeds the token panel)
  game/
    world.rs              the cobbled road to the castle gate (1,500 m), camera view, chunked scenery, potion-flask triggers, encounter zones and road-blocking monsters, region borders, lake/ground/road drawing, light pools, mist, bats and fireflies
    decor.rs              forest props (pines, gravestones, pumpkins, cottages, lampposts...) and the hub camp — per-chunk generation + drawing
    map.rs                the discovery map: furthest point reached, the regions entered, the landmarks behind it and the castle ahead (FR27)
    regions.rs            the table of regions along the road (forest, fields, village, castle grounds): names, map colours, scenery knobs (FR27)
    castle.rs             the castle at the road's end: curtain wall, gatehouse towers, portcullis, with the manor's keep behind it (FR27)
    manor.rs              the gothic manor backdrop across the lake (and on the battle horizon), its lightning, bats
    villain.rs            the mad scientist at the top of the road: movement, animation, cues, schemes and a speech bubble; lines live or from assets/villain_lines.json (FR23)
    villain_voice.rs      the doctor's live voice: one model call at a time behind the questions, a per-minute cap, a flask's two reactions asked ahead (FR23)
    gfx.rs                shared drawing helpers (night palette, ink outline, shadows, ribbons, stars, glows, vignette, potion flask, speech bubble)
    player.rs             WASD/arrow movement along the path, speed multiplier from upgrades and Shift sprint, player sprite, jump/roll/shoot
    skills.rs             roll charges and sprint stamina, and the skill bar at the bottom centre
    goalpost.rs           the goalpost tracker's state: the next thing up the road, metres left, its pop/unroll/"Now!" animation
    bow.rs                the bow: shot timing, bow and arrow drawing, cosmetic arrows that arc and stick where they land
    flicker.rs            reusable flicker look (spawn ghost, damage blink), honours "reduce flashing"
    health.rs             player HP in half-hearts (damage/heal/refill, clamped, unit-tested)
    curse.rs              the doctor's curse: a fever that starts on the road, worsens while you linger, drains hearts at full
    shop.rs               the safe stop's shop: what gears buy (hot soup, a heart charm) and the rules for a purchase
    monster.rs            monster roster (stats, rarity by distance), Species (built-in or spec) + per-monster drawing and animations
    monster_rig.rs        draws any JSON-defined monster from its body shape, parts, eyes and accessory
    battle.rs             wild-encounter battle: transition, phases, battle-view drawing (moonlit clearing)
    dialogue.rs           NPC dialogue with typewriter reveal (Professor Pebble in the hub)
    task_trigger.rs        state machine: Idle -> Loading -> Active -> Resolved
    prefetch.rs            looks up the road and asks for questions ahead of need; soft deadlines; picks the next lesson pack
    field_guide.rs         the field guide's cards, unlocked by beating monsters
    mod.rs                 Game: the one owner of run state; the per-frame update and draw, mode order, dev events
  progression/
    mod.rs                 XP, levels, streak multiplier, upgrade unlocking
    upgrades.rs             upgrade definitions (just "Quickstep Boots" for the vertical slice)
  ui/
    hud.rs                  progress bar + streak + upgrade list + unlock toast
    goalpost_panel.rs        the goalpost badge at the top centre: a circle that unrolls into "name · N m"
    creation_bar.rs          the creation bar under it: a progress circle per model call being written (FR33)
    field_guide_panel.rs     the field guide (G): a card per monster, filled in once beaten
    shop_panel.rs           the safe stop's shop (E at the cottage door)
    map_panel.rs            the map (M): discovered road, region bands, landmarks, fog, the castle
    victory_panel.rs        the screen after reaching the castle gate
    minimap.rs              the always-on minimap, top right
    settings_panel.rs        settings modal (O): panel toggles, reduce flashing, controls reference
    game_over_panel.rs       Game Over screen: run stats, try again
    dialogue_panel.rs        NPC dialogue box
    task_panel.rs            problem text, hint, answer input, submit
    token_panel.rs           model + API address, live usage rate, 60 s call timeline, recent calls, speed graph, totals
    style.rs                 the egui theme (parchment panels, dark-wood borders), applied every frame
  bin/
    gen_content.rs          offline batch generator for NPC dialogue and monsters (not called at runtime)
    validate_content.rs     offline checker: loads the dialogue bank and the content dir the way the game does (`make validate`)
openclaw/                  optional OpenClaw agent skills (FR30); the game never needs them
  README.md                setup, the agents' tool limits, Cerebras rate limits, safety
  agentctl.sh              runs agents as background jobs through an OpenClaw Gateway (logs in .agent-runs/)
  skills/jeko-dialogue/    refreshes assets/dialogue.json: runs `make gen-content`, validates, leaves the diff
  skills/jeko-monsters/    designs monster specs into content/monsters/ and validates them
  skills/jeko-playtest/    runs the JEKO_DEV / JEKO_SHOT scenarios, looks at the screenshots, reports
content/                   read at runtime and hot-reloaded (JEKO_CONTENT_DIR points elsewhere)
  monsters/*.json          monsters defined as data: four hand-written ones, plus the studio's gen-*.json (git-ignored)
  lessons/*.json           lesson packs the studio writes (git-ignored, per-machine; created on first use)
  tips/*.json              Professor Pebble's tips the studio writes (git-ignored; created on first use)
assets/                    compiled into the binary (editing one needs a rebuild)
  tasks_fallback.json      hand-written fallback math tasks (used if no API key, or the live call fails)
  dialogue.json            NPC flavor lines; Professor Pebble says one after the hub tutorial. Regenerate with gen_content
  villain_lines.json       what the mad scientist says for each cue (level-up, streak, flask right or wrong, scheme, ...) when no live line comes, with a mood and an action
```

## Setup

### With Nix (recommended — this is what was actually used to verify the build)

`flake.nix` provides a dev shell with the exact rustc/cargo plus every
runtime library macroquad needs (X11, GL, libxkbcommon, ALSA) already on
`LD_LIBRARY_PATH` — no system Rust install or manual `apt`/`nix-env` package
hunting required.

1. Set `CEREBRAS_API_KEY` one of two ways (or skip it to play offline):
   - **secretspec** (declarative, no plaintext key in a file you could
     accidentally commit): the dev shell already has the `secretspec` CLI on
     `PATH`. Run `secretspec check` once to see what `secretspec.toml`
     declares, then `secretspec config global init` to pick a backend
     (`keyring` is the default recommendation; `dotenv` and `env` also work
     if you just want something simple). `secretspec set CEREBRAS_API_KEY`
     stores it there.
   - **`.env`** (quickest): `cp .env.example .env` and put your key in
     `CEREBRAS_API_KEY`. `dotenvy` loads it at startup. Any non-empty value
     counts as a key, so don't leave the example's placeholder in place: delete
     the value to play offline.

   Set `CEREBRAS_MODEL` to a model your key can use. The default, `kimi-k3.1`,
   is a placeholder, not a verified model ID; this project has been run against
   `moonshotai-kimi-k3.1-pro-trial`. `curl -H "Authorization: Bearer $KEY"
   https://api.cerebras.ai/v1/models` lists what a key can use.
2. `make run` (or `nix develop`, then `cargo run` inside — or
   `secretspec run -- cargo run` if you went the secretspec route). See
   `make help` for every target (`info`, `build`, `release`, `check`, `fmt`,
   `lint`, `test`, `gen-content`, `gen-monsters`, `validate`, `nix-build`).

`make nix-build` (`nix build`) produces `./result/bin/jeko` as a standalone
derivation from the committed `Cargo.lock`.

Optional: `direnv allow` (if you use direnv) picks up `.envrc` so the shell
activates automatically on `cd`, without typing `nix develop` or prefixing
every command with `make`.

### Playing

| Key | Does |
|---|---|
| W / Up, S / Down | Walk up the road, back down it |
| A D / Left Right | Steer across the road |
| Shift (hold) | Sprint; drains the stamina meter on the skill bar (run it dry and you're winded until it refills) |
| E | Open the flask, challenge the monster, open the shop or talk to the person you're next to; with a problem open, jump to the answer field |
| Enter | Submit the answer (clicking the field and Submit works too) |
| Space | Jump |
| Ctrl | Roll the way you're steering (3 charges, then a short wait) |
| F | Shoot an arrow (cosmetic on the road; in a battle a right answer is shown as a bow shot) |
| M | The map of the road walked so far, with the castle at the top (pauses) |
| G | The field guide: a card for each monster you've beaten |
| P | Pause |
| O | Settings: panel toggles, reduce flashing, villain chatter, the controls (O or Esc closes) |

The run starts in Professor Pebble's camp at the forest edge (press E by
Professor Pebble for a quick tutorial); walk north through the iron gate
signed "THE ROAD". At a glowing potion flask an **E** prompt floats over it:
open it, type the number, hit Enter. You stay put until it's answered. Patches
of ferns on the road hide monsters: step in and answer questions to beat them.
Every so often a monster stands right on the road behind a glowing ward and
won't let you by: press E next to it to fight, and beat it to open the road.

Past the gate the doctor's curse sets in: it worsens while you linger, eases
as you reach new ground or solve a flask, lifts back in the camp, and drains
hearts at full. Every 250 m the road opens onto a safe stop, where the curse
lifts, hearts mend fast, nothing can hurt you, and the cottage door (E) sells
soup and heart charms for the gears you pick up along the way. Wrong answers
cost hearts, and at zero hearts it's Game Over (E/Enter to try again). Reach
the castle gate 1,500 m up to win the run.

### Dev aids

- `JEKO_SHOT=out.png:120` saves a screenshot at frame 120, prints a `STATE ...`
  line of run state to stderr, and exits.
- `JEKO_DEV=name@frame,...` fires scripted events at those frames, stepping a
  fixed 1/60 s per frame: damage, moves, battles (`jelly@150`, `battle:<id>`),
  answers through the real submit path (`right`, `wrong`), teleports (`toflask`,
  `toblocker`, `tostop`, `tocastle`, ...), the curse, the mad scientist
  (`villain:<cue>`, `say:<cue>`, `scheme:<topic>`) and more. The full list is in
  [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#11-content-configuration-and-dev-hooks).
- `JEKO_DEV=fakemodel:700` swaps the model for a stand-in that answers every
  call after 700 ms, so the live paths (look-ahead, the studio, the doctor's
  lines, the creation bar) can be tried with no key.

Example: `JEKO_SHOT=shot.png:300 JEKO_DEV=toflask@10,e@20,right@60 make run`.

### Monsters as data

Each file in `content/monsters/` is a monster (body shape, colours, eyes, parts,
an idle and an attack, a `tier` 1–4 and a `special`). Edit one while the game
runs and it updates within a second, even mid-battle; a broken edit is reported
in a toast and the last good version stays. Try
`JEKO_DEV=battle:mossmunch@5 make run`, then edit
`content/monsters/mossmunch.json`. With a key, the studio designs new ones in the
background (`gen-*.json`); they join the road from the next run. `make validate`
checks the content dir and the line banks offline, the way the game loads them.

### Configuration

Everything is optional. Values come from the environment, `.env` or secretspec.

| Variable | Default | Does |
|---|---|---|
| `CEREBRAS_API_KEY` | none | Turns on live generation; without it the game runs offline |
| `CEREBRAS_MODEL` | `kimi-k3.1` (placeholder) | The model slug (see Setup) |
| `CEREBRAS_BASE_URL` | `https://api.cerebras.ai/v1` | OpenAI-compatible endpoint |
| `JEKO_CONTENT_DIR` | `./content` | Where hot-reloaded monsters, lessons and tips live |
| `JEKO_INSTANCE` | the checkout's folder name | Label in the window title (see below) |
| `JEKO_PREFETCH` / `JEKO_PREFETCH_RPM` | on / 20 | Asking for questions ahead of need, and its calls per minute |
| `JEKO_VILLAIN` / `JEKO_VILLAIN_RPM` | on / 6 | The mad scientist's live lines (`off` uses the line bank), and their calls per minute |
| `JEKO_STUDIO` / `JEKO_STUDIO_RPM` / `JEKO_STUDIO_TOKENS` | on / 6 / 60,000 | The background studio, its calls per minute and its token budget per session |

All the live lanes share one key and its rate limit. On a trial key, expect
429s: a failed question or line falls back to its offline bank, and the studio
just tries again later.

### Running two checkouts side by side

Two clones of this repo (say `~/Development/JEKO` and `~/Development/JEKO2`) can
be built and run at the same time without touching each other. Each folder has
its own `target/`, `.env`, `.direnv/` dev shell, `content/` and git branch, and the
game has no ports, save files or other shared state (assets are compiled in).

- **Telling them apart.** The window title ends in the instance name, e.g.
  `JEKO — Math Quest [JEKO2]`, and the game prints `jeko instance JEKO2 (built
  from …)` to stderr on startup. The name is the folder the binary was built
  from. Set `JEKO_INSTANCE` (in the environment or that folder's `.env`) to
  override it.
- **Checking a folder.** `make info` prints the instance, folder, branch and
  commit, build dir, and whether `.env` has an API key. A folder with no `.env`
  quietly runs on the offline fallback bank, so check this when the two behave
  differently.
- **Setting up a new folder.** `cp ../JEKO/.env .env` (or `secretspec`, which is
  per-user and so already shared), `direnv allow`, then `make run`. The first
  build there is a full one, since `target/` isn't shared. Don't point both at
  one `CARGO_TARGET_DIR`. They'd overwrite each other's binary.
- **Running both.** Use `make run` in a terminal per folder. Both use the same
  API key, so they share its rate limit.

### OpenClaw agents (optional, not part of the game)

`openclaw/` holds skills that let [OpenClaw](https://docs.openclaw.ai) agents
do JEKO's offline jobs: refresh the NPC dialogue, design monster specs, and
playtest through the dev hooks. They only run the repo's own tools and leave
their results in the working tree for review; nothing in `src/` knows they
exist. `openclaw/agentctl.sh` runs them as background jobs. Setup, tool
limits and safety notes are in [openclaw/README.md](openclaw/README.md).

`nix develop .#openclaw` drops into a separate shell with OpenClaw on `PATH`.
It's kept out of the default shell deliberately: nixpkgs marks `openclaw` as an
insecure package — not for an unpatched CVE, but as a standing classification,
because it gives an LLM full system access by default and parses untrusted
content, so it's vulnerable to prompt injection. The `openclaw` devShell scopes
`permittedInsecurePackages` to just that shell (see `flake.nix`) so running the
normal game dev shell never pulls it in.

### Without Nix

1. Install Rust via [rustup](https://rustup.rs).
2. On Linux you'll additionally need the X11/GL/ALSA runtime libs on your
   system (on NixOS use the flake instead; on Debian/Ubuntu something like
   `libx11-dev libxi-dev libxcursor-dev libxrandr-dev libxinerama-dev
   libxkbcommon-dev libasound2-dev libgl-dev` covers it).
3. `cp .env.example .env`, fill in `CEREBRAS_API_KEY`, then `cargo run`.

If you skip the API key entirely, the game still runs fully offline using
`assets/tasks_fallback.json`.

### A dependency trap already hit and fixed here

`egui-macroquad = "0.15"` pulls in an old `egui-miniquad` that transitively
depends on **macroquad 0.3**, while this project depends on **macroquad
0.4** directly — linking both into one binary fails with `multiple
definition of 'CONTEXT'` (macroquad keeps a global context as a plain
static). `egui-macroquad = "0.17"` (pinned in `Cargo.toml`) depends on
macroquad `^0.4.14` directly and has no such conflict — don't downgrade it
without re-checking this.

## The incentive loop (why a student keeps playing)

- **Progress bar** — always-visible XP bar toward the next level
  (`ui/hud.rs`), so "how close am I" is never a question.
- **Goalpost tracker** — a badge at the top centre names the next thing up the
  road (a flask, ferns, a monster, a milestone, a safe stop, a new region, the
  Tide Titan, the castle) and counts down
  the metres to it (`ui/goalpost_panel.rs`, `game/goalpost.rs`).
- **Gears and safe stops** — brass gears lie in trails along the road, and every
  250 m the road opens onto a lantern-lit clearing with a cottage. There the fever
  lifts, hearts mend fast and nothing can hurt you, and the cottage door (E) sells
  hot soup and heart charms for gears (`game/world.rs`, `game/shop.rs`). Gears and
  hearts bought are lost on death, so it's worth spending them.
- **The mad scientist** — past the gate, Dr. Nullwell potters around at the top
  of the road, always just out of reach, and reacts to what you do with a speech
  bubble: a right answer flusters them, a level-up or a streak makes their frizz
  stand on end (`game/villain.rs`). With an API key Cerebras writes their lines
  live from what just happened in the run, behind the questions and capped at
  `JEKO_VILLAIN_RPM` (6) a minute; with none, or with `JEKO_VILLAIN=off`, they use
  the reviewed lines in `assets/villain_lines.json`. Every 300 m they announce a
  scheme ("Operation Fraction Fog") with a gadget on their cart, and the next three
  flasks ask about its topic. When the studio finishes a monster they show it off.
  Apart from a scheme's topic, nothing they do changes the game. Settings has a
  "Villain chatter" switch.
- **Creation bar** — circles at the top centre show the model writing as it
  happens: live questions, look-ahead questions, the doctor's lines, and the
  studio's monsters, lesson packs and tips. A ring turns while it writes, closes when it lands, and
  shows how long it took, and a line under it says what is already made and
  waiting (`ui/creation_bar.rs`, `llm/activity.rs`). Settings can hide it.
- **Streak multiplier** — consecutive correct answers raise XP gain up to
  +100% (`Progression::streak_multiplier`). Getting one wrong resets it to
  zero — this is what makes stopping mid-streak feel costly, which is the
  actual hook that keeps someone going "just one more."
- **Upgrade unlocked by XP, not bought** — "Quickstep Boots" (+25% move
  speed) unlocks automatically at a lifetime-XP threshold, with a toast
  announcing it. It's tied to total effort, not a single success, so it
  can't be gamed by one easy problem (gears buy only soup and hearts).
- **Live, varied problems** — because each task is freshly generated
  (subject to difficulty scaling off player level), the content doesn't
  visibly repeat the way a static problem bank would. Every generated answer
  is checked against its own arithmetic before it's shown.
- **Learning from mistakes** — a wrong answer shows a worked explanation
  beside the next question, and the studio writes practice packs on the topic
  the player keeps missing (`content/lessons.rs`, `game/prefetch.rs`).
- **Somewhere to get to** — the road runs through regions with their own look
  to the castle gate, 1,500 m up, and the map (M) fills in as you walk it
  (`game/regions.rs`, `game/map.rs`). Beaten monsters fill the field guide (G).

## What's next

The backlog, with what's built and what's left, is
[FeatureRequestToDo.md](FeatureRequestToDo.md). A few smaller ideas that
aren't filed there:

- More upgrades: a dash, double-jump, or cosmetic skin — `progression/upgrades.rs`
  is the only file that needs a new entry; the unlock logic is generic.
- Escalating difficulty with distance walked — questions currently scale only
  with player level (plus a monster's bias), capped at 5.
- A save file (serde to a local JSON) so progress persists across runs. Nothing
  is saved today; keeping learner data needs the owner's privacy decision (FR14, FR16).
- Swapping the flat-color primitives in `game/decor.rs`, `game/world.rs`,
  and `game/player.rs` for real sprite textures — the lift-and-shadow and
  y-sorted drawing already in place will read as much more convincingly
  "2.5D" once there's actual art sitting on top of it.

## Known environment quirk

In an audio-less sandbox (e.g. WSL without a configured audio device, or a
bare container), macroquad's audio backend panics during startup
(`Can't open PCM device` from `quad-snd`'s ALSA init) and takes the whole
process down with it — this is not a background-thread warning you can
ignore, the window never opens. Workaround for local dev: point ALSA at a
null device before running, e.g.

```
printf 'pcm.!default { type null }\nctl.!default { type null }\n' > /tmp/asound.conf
ALSA_CONFIG_PATH=/tmp/asound.conf cargo run
```

If you're demoing from inside a container, confirm `/dev/snd` (or
Pulse/Pipewire) is actually reachable beforehand so it isn't a surprise on
stage.

**Audio is explicitly out of scope right now** — there's no sound design,
no SFX/music assets, and no abstraction over `quad-snd` in this codebase.
Don't spend time hardening or routing around the ALSA panic as a feature;
the null-device workaround above is only for getting the window open in a
silent dev environment. Revisit audio only if it's reintroduced as an
explicit goal for the vertical slice.
