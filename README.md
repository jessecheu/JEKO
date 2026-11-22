# JEKO — Math Quest

A 2.5D exploration game set in a dark forest at night: you walk up an
endless, winding cobbled road past a black lake, with a mad scientist's
gothic manor looming across the water. Opening a glowing potion flask (walk
up, press E) asks Cerebras-hosted
Kimi K3.1 to write a fresh, difficulty-scaled math problem on the spot. Built
for a hackathon demo of Cerebras' low-latency inference: the problem should
appear fast enough that it doesn't break the player's movement flow.

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
- **serde / serde_json** — parsing the model's JSON task payload and the
  local fallback bank.

## Project layout

```
src/
  lib.rs                 shared library (so src/main.rs and src/bin/gen_content.rs can both use it)
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
    mod.rs                 ties player/world/progression/trigger together
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
  bin/
    gen_content.rs          offline batch generator for NPC dialogue and monsters (not called at runtime)
    validate_content.rs     offline checker: loads the dialogue bank and the content dir the way the game does (`make validate`)
openclaw/                  optional OpenClaw agent skills (FR30); the game never needs them, see openclaw/README.md
  skills/jeko-dialogue/    refreshes assets/dialogue.json: runs `make gen-content`, validates, leaves the diff
  skills/jeko-monsters/    designs monster specs into content/monsters/ and validates them
  skills/jeko-playtest/    runs the JEKO_DEV / JEKO_SHOT scenarios, looks at the screenshots, reports
content/
  monsters/*.json          monsters defined as data, loaded at runtime and hot-reloaded while the game runs
  lessons/*.json           lesson packs the studio writes (a per-machine cache; created on first use)
  tips/*.json              Professor Pebble's tips the studio writes (created on first use)
assets/
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

1. Set `CEREBRAS_API_KEY` one of two ways:
   - **secretspec** (declarative, no plaintext key in a file you could
     accidentally commit): the dev shell already has the `secretspec` CLI on
     `PATH`. Run `secretspec check` once to see what `secretspec.toml`
     declares, then `secretspec config global init` to pick a backend
     (`keyring` is the default recommendation; `dotenv` and `env` also work
     if you just want something simple). `secretspec set CEREBRAS_API_KEY`
     stores it there.
   - **`.env`** (quickest): `cp .env.example .env` and fill in
     `CEREBRAS_API_KEY`. `dotenvy` loads it at startup same as before.

   Either way, confirm the exact model slug for Kimi K3.1 on Cerebras from
   the hackathon docs and set `CEREBRAS_MODEL` accordingly — `kimi-k3.1` in
   `.env.example`/`secretspec.toml` is a placeholder, not a verified model ID.
2. `make run` (or `nix develop`, then `cargo run` inside — or
   `secretspec run -- cargo run` if you went the secretspec route). See
   `make help` for every target (`build`, `release`, `check`, `fmt`, `lint`,
   `test`, `gen-content`, `gen-monsters`, `validate`, `nix-build`).
3. Controls: W/Up walks up the road (the map scrolls with you, until the castle gate 1,500 m up),
   A/D or Left/Right steer across it, S/Down backs up; hold **Shift** to
   sprint, which drains the stamina meter on the skill bar at the bottom of the
   screen (run it dry and you're winded until it refills). Walk up to a glowing
   potion flask and an **E** key prompt floats over it; press E to
   open its task (you stay put until it's answered). Press E again to jump
   into the answer field, type the number, and hit Enter. **M** opens the map of the road you've walked, with the castle waiting at the top (the world pauses). Walk all the way to the castle gate to win the run. At a safe stop's cottage (every 250 m), **E** opens the gear shop. No mouse needed
   (clicking the field and Submit still works). **Space** jumps,
   **Ctrl** rolls the way you're steering (3 in a row, then a short wait) and **F** shoots an arrow from your bow (the
   shot is cosmetic for now; in a battle, a right answer is shown as a bow shot). **P** pauses,
   **G** opens the field guide (a card for each monster you've beaten) and **O** opens settings
   (O or Esc closes them).
   The run starts in Professor Pebble's camp at the forest edge (press E by
   Professor Pebble for a quick tutorial); walk north through the iron gate
   signed "THE ROAD". Patches of ferns on the road hide monsters:
   step in and answer questions to beat them. Every so often a monster
   stands right on the road behind a glowing ward and won't let you by:
   press E next to it to fight, and beat it to open the road. Past the gate the doctor's
   curse sets in: it worsens while you linger, eases as you reach new
   ground or solve a flask, lifts back in the camp, and drains hearts at
   full. Wrong answers cost hearts,
   and at zero hearts it's Game Over (E/Enter to try again).

   Dev aids: `JEKO_SHOT=out.png:120` saves a screenshot at frame 120 and
   exits; `JEKO_DEV=jelly@150,wrong@270` fires scripted events (`hurt`,
   `kill`, `restart`, `jump`, `roll`, `shoot`, `stun`, `cheer`, `spent`, `pause`, `settings`,
   battles `pinch`/`jelly`/`puff`/`coco`/`gull`/`wyrm`/`titan`, `battle:<id>`
   for a monster from `content/monsters/`, `right`/
   `wrong` answers through the real submit path, `e` presses E, and
   teleports `toflask`/`tozone`/`toblocker`/`topebble`/`toroad`/`neargoal`/`atgoal`, `nohud` hides the side panels,
   `calm` turns on reduce flashing, `curse`/`cursefull` set the curse to
   the fever stage or full, `tovillain` puts the doctor on screen,
   `villain:<cue>` / `say:<cue>` make them react from the bank or the live path,
   `scheme:<topic>` starts a scheme, `nochatter` hides their bubble) at those frames, stepping a
   fixed 1/60 s per frame. At the shot it prints a `STATE ...` line of run
   state to stderr.

   Monsters as data: each file in `content/monsters/` is a monster (body shape,
   colours, eyes, parts, an idle and an attack, a `tier` 1–4 and a `special`).
   Edit one while the game runs and it updates within a second, even mid-battle;
   a broken edit is reported in a toast and the last good version stays. Try
   `JEKO_DEV=battle:mossmunch@5 make run`, then edit
   `content/monsters/mossmunch.json`. `JEKO_CONTENT_DIR` points the game at
   another folder.

Optional: `direnv allow` (if you use direnv) picks up `.envrc` so the shell
activates automatically on `cd`, without typing `nix develop` or prefixing
every command with `make`.

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

### openclaw (optional, not part of the game)

`nix develop .#openclaw` drops into a separate shell with
[OpenClaw](https://docs.openclaw.ai) on `PATH`, for anyone who wants a
personal AI-assistant gateway available alongside this repo. It's kept out of
the default shell deliberately: nixpkgs marks `openclaw` as an insecure
package — not for an unpatched CVE, but as a standing classification, because
it gives an LLM full system access by default and parses untrusted content,
so it's vulnerable to prompt injection. The `openclaw` devShell scopes
`permittedInsecurePackages` to just that shell (see `flake.nix`) so running
the normal game dev shell never pulls it in.

`nix build` also works once `Cargo.lock` exists (it's generated the first
time you run `cargo build`/`make build`, and is committed to the repo here) —
it produces `./result/bin/jeko` as a standalone derivation.

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
  road (a flask, ferns, a monster, a milestone, the Tide Titan) and counts down
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
  happens: live questions, look-ahead questions, and the studio's monsters,
  lesson packs and tips. A ring turns while it writes, closes when it lands, and
  shows how long it took, and a line under it says what is already made and
  waiting (`ui/creation_bar.rs`, `llm/activity.rs`). Settings can hide it.
- **Streak multiplier** — consecutive correct answers raise XP gain up to
  +100% (`Progression::streak_multiplier`). Getting one wrong resets it to
  zero — this is what makes stopping mid-streak feel costly, which is the
  actual hook that keeps someone going "just one more."
- **Upgrade unlocked by XP, not by a shop** — "Quickstep Boots" (+25% move
  speed) unlocks automatically at a lifetime-XP threshold, with a toast
  announcing it. It's tied to total effort, not a single success, so it
  can't be gamed by one easy problem.
- **Live, varied problems** — because each task is freshly generated
  (subject to difficulty scaling off player level), the content doesn't
  visibly repeat the way a static problem bank would.

## Stretch goals (not in the vertical slice)

- More upgrades: a dash, double-jump, or cosmetic skin — `progression/upgrades.rs`
  is the only file that needs a new entry; the unlock logic is generic.
- Escalating difficulty with distance walked — flasks currently scale only
  with player level.
- A second task type (geometry, fractions) via a second prompt template in
  `llm/task_gen.rs`.
- Putting the other NPCs from `assets/dialogue.json` (Wanderer Finch, Old
  Lantern) on the road for flavor text between tasks. Only Professor Pebble,
  in the hub, uses the bank so far.
- A save file (serde to a local JSON) so progress persists across runs.
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
