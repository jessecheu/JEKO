---
type: Reference
title: JEKO — Architecture
description: As-built overview of JEKO's architecture — crate layering, the frame loop, the Game state owner and its modes, the live-question pipeline to Cerebras, procedural world generation, 2.5D rendering, battles, progression, the egui layer — and the invariants a change has to keep.
tags: [architecture, design, overview, rust, macroquad, egui, cerebras]
timestamp: 2026-11-21
---

# JEKO — Architecture

> **Bottom line.** JEKO is one Rust crate: a library plus two binaries. A single
> macroquad loop in `src/main.rs` drives one owner of all run state, `game::Game`,
> which updates and paints the world each frame. egui panels are drawn on top as
> thin views of that state. Only one thing is slow: asking Cerebras for a math problem.
> That call runs on a throwaway OS thread and its result comes back over an `mpsc` channel
> that `TriggerState` polls each frame. When there's no key or the call fails, a fallback
> bank compiled into the binary steps in. The world isn't stored as a map. The path is a function of `y`, and each chunk's
> scenery, potion flask and monster zone is a pure function of the chunk index, so the
> road is endless, the same every run, and unit-testable. The setting is a dark forest
> at night (FR24), with the mad scientist's manor drawn as a backdrop across the lake.

This doc is **descriptive**. It says what the code is today, checked against the source
on the date in the frontmatter. It isn't a roadmap: planned and open work lives in
[FeatureRequestToDo.md](../FeatureRequestToDo.md), and setup, controls and the file
tree live in [README.md](../README.md). **When a change makes anything here untrue,
update this doc in the same change.** [AGENTS.md](../AGENTS.md#keeping-the-architecture-doc-current)
says when and how.

---

## 1. System at a glance

```
src/main.rs: one macroquad loop, one iteration per frame
  │
  ├─ Game::update(dt) ─────────────┐
  ├─ Game::draw()                  │      game::Game  (the one owner of run state)
  ├─ Game::draw_pause_overlay()    ├───▶  player · world · battle · dialogue · health
  └─ egui pass: ui::* panels ──────┘      progression · settings · trigger_state
        read Game, hand events                  │
        back (Submit, restart)                  │ Game::request_task(bias)
                                                ▼
                              llm::CerebrasClient::request_task_async(difficulty)
                                spawns a std::thread ──▶ POST {CEREBRAS_BASE_URL}/chat/completions
                                returns an mpsc::Receiver        (blocking reqwest, 8 s timeout)
                                                │
                   TriggerState::Loading(rx) ── try_recv() each frame ──▶ Active(MathTask)
                                                │ error, or no API key
                                                └──▶ random task from assets/tasks_fallback.json
```

*Figure: the frame loop and the one asynchronous path, to Cerebras.*

---

## 2. Crate layout and layering

One Cargo package, `jeko` (`Cargo.toml`, edition 2021):

| Target | Source | Role |
|---|---|---|
| library `jeko` | `src/lib.rs` | `config`, `llm`, `game`, `progression`, `ui`. It exists so both binaries share `CerebrasClient`. |
| binary `jeko` (default-run) | `src/main.rs` | The game: window config, the frame loop, the dev hooks (§11). |
| binary `gen_content` | `src/bin/gen_content.rs` | Offline tool. Asks the model for NPC lines and writes `assets/dialogue.json`. The game never runs it. |

Which module uses which (arrows mean "uses"):

```
main.rs ──▶ ui ──▶ game ──▶ llm ──▶ config
            │        └────▶ progression
            └──────────────▶ progression, llm   (read-only, for the HUD and token panel)
gen_content.rs ──▶ llm, config
```

The layering holds today, and new code should keep it:

- **`config`, `llm`, `progression`** know nothing about macroquad or egui. They're
  plain Rust, so they can be tested and reused without a window (`gen_content` uses
  `llm` this way).
- **`game`** uses macroquad (input, drawing, render targets) and never egui. It
  mentions egui only in comments.
- **`ui`** is the only module that imports egui. Its panels are functions, not
  stateful widgets (§9).

| Module | Owns |
|---|---|
| `config.rs` | `Config::load()`: reads `CEREBRAS_API_KEY`, `CEREBRAS_MODEL`, `CEREBRAS_BASE_URL` from the environment, after `dotenvy` loads `.env`. An empty key counts as no key. `instance_name()`: `JEKO_INSTANCE`, else the folder the binary was built from (`CARGO_MANIFEST_DIR`), shown in the window title. |
| `llm/mod.rs` | `MathTask { topic, problem, answer: f64, hint, difficulty }`, the one task type every question path produces. |
| `llm/cerebras_client.rs` | `CerebrasClient`: the async wrapper, the blocking chat-completions call, and the shared token stats. |
| `llm/task_gen.rs` | The prompt template (`build_prompt(difficulty)`) and lenient JSON extraction (`extract_json_object` / `_array`). |
| `llm/usage.rs` | `RequestUsage` (one call's tokens and speed) and `TokenStats` (session totals plus the last 24 requests). |
| `game/mod.rs` | `Game`: run state, the per-frame update and draw, the mode precedence, the damage, answer and reset entry points, and the dev events. |
| `game/task_trigger.rs` | `TriggerState`, the question state machine (§4), and `pick_fallback`. |
| `game/world.rs` | Coordinates and camera (`View`), path geometry, chunking, potion flasks (`Spot`), encounter zones, the depth-sorted `Tall` items, and drawing the ground, the cobbled road, the light pools, the mist, the bats and fireflies, and where the manor sits on screen. |
| `game/decor.rs` | Per-chunk forest scenery generation (`generate_chunk`), the hand-placed hub camp (`hub_layout`), how each prop draws, and which props cast light (`Decoration::lights`). |
| `game/manor.rs` | The gothic manor backdrop (`draw`, with an optional lake reflection), its lightning timing (`lightning`), and the shared `bat` silhouette. |
| `game/gfx.rs` | The shared drawing vocabulary: the `INK`, `SHADOW`, `MOON`, `LANTERN`, `LAB_GREEN` and `NIGHT` colours, shading, ellipses, ribbons, stars, gradient meshes (`gradient_ring`, `glow`, `vignette`), the potion flask, hearts, the key prompt, the item-get burst, and `hash01`. |
| `game/player.rs` | `Player`: movement, the cosmetic `Action`s (jump, roll, shoot), the arrows it has loosed, its `Skills`, stun, cheer, death, the stolen hat, and the sprite drawn through a render target. |
| `game/skills.rs` | Movement skills (FR26): `RollCharges` (3 rolls, a trickle, a lockout) and `Stamina` (sprint drain, regen, winded), held together in `Skills`, and `draw_bar`, the skill bar at the bottom centre. |
| `game/bow.rs` | The bow: the shot's timing (`aim`, `SHOOT_S`, `RELEASE_S`), drawing the bow slung and held and an arrow (`draw_arrow`), and `Arrow`, a cosmetic projectile that arcs, then sticks in the ground or splashes in the lake. |
| `game/flicker.rs` | `Flicker` / `FlickerKind` (spawn ghost, damage blink). It turns elapsed time into a `Look` (tint and alpha) and honours reduce flashing. |
| `game/health.rs` | `Health`, counted in half-hearts: damage, heal, refill, clamped. |
| `game/monster.rs` | The `MonsterKind` roster, `MonsterInfo` stats, distance-weighted `pick`, and per-monster drawing and animation. |
| `game/battle.rs` | `Battle` and its `Phase`s, the iris transition, and the battle-view drawing. |
| `game/dialogue.rs` | `Dialogue` (typewriter reveal), the `PEBBLE_TUTORIAL` lines, and `load_bank()` for `assets/dialogue.json`. |
| `progression/` | `Progression` (XP, level curve, streak multiplier, upgrade unlocks) and `upgrades.rs` (upgrade definitions). |
| `ui/` | One egui function per panel (`hud`, `task_panel`, `token_panel`, `settings_panel`, `dialogue_panel`, `game_over_panel`), plus `style::apply`. |

---

## 3. The frame loop (`src/main.rs`)

macroquad's `#[macroquad::main]` gives an `async fn main`, but `next_frame().await`
only yields once per frame. **It isn't an executor that can drive other futures**,
which is why network work goes to a thread (§4). Each iteration does this, in order:

1. `clear_background` (`gfx::NIGHT`).
2. Fire any `JEKO_DEV` events scheduled for this frame (`Game::dev_event`).
3. Choose `dt`: a fixed 1/60 s when `JEKO_SHOT` or `JEKO_DEV` is set, so frame numbers
   map to game time; `get_frame_time()` otherwise.
4. `game.update(dt)`: input, simulation, polling the question (§5).
5. `game.draw()`: the world or the battle view, in screen space (§7).
6. `game.draw_pause_overlay()`: the pause dim, or the dim behind Game Over.
7. **egui pass** (`egui_macroquad::ui`): `ui::style::apply`, then set
   `game.typing = ctx.wants_keyboard_input()`, then draw the panels. The HUD and token
   panel are hidden during a battle or dialogue. The task panel returns
   `TaskPanelEvent::Submit`, which calls `game.submit_answer()`. The Game Over panel
   returns `true`, which calls `game.reset()`.
8. `egui_macroquad::draw()`, then the `JEKO_SHOT` screenshot check, then `next_frame().await`.

Two consequences of this order:

- **`typing` lags one frame.** It's set in step 7 and read by the next step 4. That's
  what keeps P, O and E from acting as game keys while the answer field has focus.
- **Answers are submitted from inside the egui closure**, after that frame's update.
  `submit_answer` mutates `Game` directly. The world reacts on the next frame.

---

## 4. `Game`: state owner and modes (`game/mod.rs`)

Everything that changes during a run is a field on `Game`, or is owned by one: `player`,
`world`, `progression`, `health`, `trigger_state`, `battle`, `dialogue`, `settings`,
`run` stats, the camera (`cam_y`, `shake`), toasts, and the game clock `time`. The UI
gets it by reference. Nothing else holds gameplay state.

`Game::update` runs these checks in a fixed order. The first one that matches decides
what the frame does:

| # | Mode | Condition | What the frame does |
|---|---|---|---|
| 1 | **Game over** | `game_over.is_some()` | Only the death animation advances. After `DEATH_ANIM_S` (1.2 s), E or Enter calls `reset()`. |
| 2 | Toggles | not `typing` | O toggles `settings_open`, and Esc closes it. With settings closed, P toggles `paused`. |
| 3 | **Paused** | `paused \|\| settings_open` | Only `trigger_state.poll()` runs, so a response that arrives isn't lost. The clock doesn't advance. |
| 4 | (every unpaused frame) | | Advance `time`, shake, the toast and the item-get timers. |
| 5 | **Battle** | `battle.is_some()` | `update_battle`: the player is held, phases advance (§8), and new questions are requested. |
| 6 | **Dialogue** | `dialogue.is_some()` | The player is held. E reveals the line, then advances, then closes. |
| 7 | **Road** | otherwise | See below. |

The road frame, in order:

1. **Busy check.** While `trigger_state` is `Loading` or `Active`, the player is held
   (`player.stop`). Each flask is a checkpoint, so a fast model means a short stop.
2. Otherwise `player.update(dt, progression.speed_multiplier(), world::clamp_to_path)`.
3. The camera eases toward the player (anchor 0.66 of screen height, rate `1 − e^(−6·dt)`).
   `world.ensure_generated` keeps one chunk past the top of the screen generated.
4. `trigger_state.poll(&fallback_tasks)`.
5. If not busy, stepping into an uncleared encounter zone starts a battle.
6. Interaction targets: the nearest fresh flask in reach, or else Professor Pebble.
   E opens the flask, talks to Pebble, or focuses the answer field when a task is open.
7. A `Resolved` cooldown counts down to `Idle`.

**Single entry points.** Several systems have exactly one door, so there's one place
to change each rule:

- `Game::damage_player(halves, from)` is the only way HP goes down. It ignores hits
  while dead or while a flicker is active, which is the invulnerability window. It
  starts the damage flicker (`HURT_INVULN_S` = 1 s), knocks the player back 22 px away
  from `from` (clamped to the path), shakes the screen unless reduce flashing is on,
  and calls `die()` at 0 HP.
- `Game::request_task(bias)` is the only way a question starts (§5).
- `Game::submit_answer()` is the only way an answer is judged, for flasks and battles alike.
- `Game::reset()` starts a new run without relaunching. It keeps the client, the
  fallback bank, the settings and `best_distance_m`. Everything else is rebuilt, and
  dropping `trigger_state` abandons any request in flight.

**The game clock.** `Game.time` advances only on unpaused frames, and every animation
reads it. Lanterns, flasks, lightning, mist and monsters therefore freeze with a pause.
Animations must use it, not wall time.

---

## 5. The question pipeline (flasks and battles)

The game's reason to exist is that a fresh, difficulty-scaled problem shows up while
the player is still moving. Potion flasks and battles share one path:

```
            open_spot / battle phase wants a question
                              │
                   Game::request_task(bias)
                   difficulty = min(level + bias, 5)
                 ┌────────────┴─────────────┐
           API key set                  no API key
                 │                          │
   Loading(request_task_async)     Active(pick_fallback)
                 │ poll() each frame
      ┌──────────┼─────────────────────┐
  Ok(task)    Err(_) / Disconnected   Empty
      │          │                     └─ keep waiting
  Active(task)  Active(pick_fallback)
                 │
     submit_answer(): parse input as f64, correct when |given − answer| < 1e-6
                 │
  Resolved { correct, cooldown }  ── cooldown hits 0 ──▶ Idle
   (flask: 1.5 s; battle: f32::MAX, replaced when the battle asks its next question)
```

*Figure: `TriggerState` (`game/task_trigger.rs`) from request to resolution.*

**The request** (`CerebrasClient`, `llm/cerebras_client.rs`):

- `request_task_async` clones the client, spawns a `std::thread`, and returns the
  `Receiver`. The thread runs `request_task_blocking`. There's no pool and no
  cancellation. A dropped receiver just makes the thread's `send` fail quietly.
- `request_raw_blocking(system, user)` is shared with `gen_content`. It builds a new
  `reqwest::blocking::Client` with an 8 s timeout and POSTs
  `{model, messages: [system, user], temperature: 0.7}` with bearer auth to
  `{base_url}/chat/completions`. It returns `choices[0].message.content`.
- **The prompt** (`task_gen::build_prompt`) asks for *only* one JSON object of the
  form `{topic, problem, answer: number, hint, difficulty}`, with a single numeric
  answer. Difficulty 1 means single-digit addition and 5 means multi-step word problems.
  `extract_json_object` slices from the first `{` to the last `}`, so stray prose or
  code fences around the object don't matter. serde then parses it into `MathTask`.
- **Usage accounting.** Every clone shares one `Arc<Mutex<TokenStats>>`. A response
  with a `usage` block is recorded. Tokens per second come from Cerebras'
  `time_info.completion_time` when present, and from the HTTP round trip otherwise.
  The round trip reads lower, and the panel says which one it used. A transport or
  HTTP error is counted with `record_failure`. A poisoned lock is ignored, because the
  numbers only feed a readout.

**The fallback bank.** `assets/tasks_fallback.json` is compiled in with
`include_str!` in `main.rs` and parsed at startup, where a malformed file panics.
`pick_fallback` panics if the bank is empty.
`pick_fallback` takes a random entry (`rand::thread_rng`, the only non-deterministic
randomness in the game). The game must stay fully playable with no key and no network.

**Answers and their effects** (`Game::submit_answer`):

| | Correct | Wrong |
|---|---|---|
| **Flask** | `record_correct(20)` (times the streak multiplier). Heals ½ heart, or refills on a level-up. Cheer pose and a "+N XP!" banner. The spot is marked `Solved`. | `record_incorrect()` resets the streak. `damage_player(2, None)` costs 1 heart. The spot is marked `Missed`. |
| **Battle** | `record_correct(xp_per_hit)`; a level-up refills. Phase → `PlayerAttack`. | `record_incorrect()`. Phase → `MonsterAttack`, whose blow does the damage. |

A stunned player's submit is refused with a toast. Input that won't parse as a number
is ignored, and the buffer is kept.

---

## 6. World model (`game/world.rs`, `game/decor.rs`)

**Coordinates.** Plain pixels in a 3/4 top-down view: +x is right and **+y is down
the screen**, so walking "forward" up the road means `y` decreasing without
bound. `View` is a pure translation that centres the 960 px `WORLD_W` strip
horizontally and rounds to whole pixels. A wider window just shows more lake and
woods. Height is a straight upward lift in screen pixels.

**The setting** (FR24). The road is an old raised cobbled causeway with iron fences
and lanterns. A black lake lies to the left (west) and the woods to the right
(east), with open forest floor between each and the road: the *lakeside* (graves,
pumpkins, giant toadstools) and the *verge*. The question pickups are potion flasks
hovering over rune stones; monsters hide in patches of ferns.

**Geometry as functions of `y`.** Nothing is a tiled map:

- `path_center(y)`: two slow sines, ramped in from dead straight over the first
  700 px, with a maximum slope of about 0.25 so the fence nudges rather than shoves.
- `bank_x(y)` (where the lake meets dry land) and `treeline_x(y)` (where the woods
  start) follow the path with their own wobble, so the ground never pinches shut.
- `walkable_x(y)` is the road's width (`PATH_HALF_W − WALK_MARGIN`) north of the
  gate. South of it, the width eases out over a 150 px funnel into the hub's open ground.
- `clamp_to_path(p)` is the only movement constraint. It's passed into `Player::update`
  as a closure and also used for knockback.

**Landmarks** (all on the y axis):

```
  y → −∞   … the road, endless …            distance_m = (ROAD_START_Y − y) / 24
  y = 150  ROAD_SOUTH_END = ROAD_START_Y: the iron gate, signed "THE ROAD"
  y = 300  funnel ends
  y = 365  Professor Pebble (pebble_pos)
  y = 420  PLAYER_START_Y (spawn)
  y = 880  SOUTH_LIMIT  (HUB_SOUTH − 30)
```

South of the gate is the **spawn hub**, a safe zone dressed as Professor Pebble's
lantern-lit camp (a campfire, a covered wagon, a cottage, lampposts). No encounters
are placed there, and it's laid out by hand in `decor::hub_layout`.

**Chunks.** The world is cut into 320 px strips (`CHUNK_H`). Chunk `k` spans
`y ∈ [−(k+1)·320, −k·320]`, and chunk 0 starts at y = 0. Chunks run from `FIRST_CHUNK`
= −4, which fills the screen below the hub, and are generated lazily by
`World::ensure_generated(north_y)`. **They're never dropped.** They're small, and
flasks and zones must remember being solved if the player walks back. Each chunk
gets:

| What | Where decided | Rule |
|---|---|---|
| Scenery | `decor::generate_chunk(k, spot)` | `StdRng` seeded from `k`. Props go in zones relative to the path and the bank (lake, shallows, bank, lakeside, verge, woods, plus `Far*` zones only a wide window shows), and never on the road or around the flask. A pair of lampposts frames each flask, and a stone milestone stands every 50 m. |
| A potion flask (`Spot`) | `world::spot_for_chunk(k)` | Even chunks with `k ≥ 0`, positioned with `gfx::hash01`. Chunk 0's flask is fixed just past the gate, near the middle of the road. Its potion colour (`flask_color`) is a hash of its position. |
| An encounter zone | `world::encounter_for_chunk(k)` | Odd chunks with `k ≥ 1`. A Tide Titan always guards the first odd chunk at or past each 500 m mark. Otherwise there's a 70% chance of a zone, with the monster from `MonsterKind::pick(roll, meters)`. |

Because flasks take even chunks and zones take odd ones, they never crowd each
other. The same seed gives the same world on every run and every restart.

---

## 7. Rendering: the 2.5D look

Everything is flat-coloured macroquad primitives. There are no textures yet. The
depth comes from three things: lifted surfaces (the road sits `ROAD_LIFT` = 6 px
up), cool cast shadows (`gfx::SHADOW`), and **back-to-front sorting by ground `y`**.
The night (FR24) comes from a dark palette, pools of light, and a vignette.

`Game::draw` paints the road view in this order:

1. `world.draw_ground`: the lake, the muddy bank, the forest floor, the woods' darker
   ground, then flat decor (speckles, leaves, paw prints, toadstools, lily pads, water
   ripples, will-o'-wisps).
2. `world.draw_manor`: the manor backdrop (`manor::draw` with its reflection). It is
   **pinned to the screen**, not placed in the world: its x follows the path's slow
   meander so it stays over the lake, and its y drifts with the camera, so it's
   always ahead. Lightning flashes behind it every ~9 s (`manor::lightning`, timed by
   `hash01` on the game clock).
3. `world.draw_road`: the cobbles and curbs, the fern patches of encounter zones,
   and the rune stones under the flasks.
4. `world.draw_lights`: soft light pools (`gfx::glow`) on the ground under lampposts,
   fence lanterns, lit pumpkins, the campfire, cottage windows, cauldrons and the
   waiting flasks. Props say what light they cast in `Decoration::lights`.
5. **The depth-sorted pass.** `world.tall_items(&view)` returns every `Tall` item on
   screen: `Decor` (pines, dead trees, gravestones, cottages, …), fence `Post`s with
   their rails (every third pair with a lantern), and `Flask`s. The player and their
   arrows (keyed by the ground under them) are added, everything is sorted by
   `sort_y()`, and the items are drawn in order. The player walks behind a tree north
   of them and in front of one south of them. The sort is stable and arrows go in
   after the player, so a fresh arrow draws over the archer.
6. `world.draw_mist`: faint drifting mist banks.
7. `gfx::vignette` around the player's feet: clear within 190 px, fading to half of
   `NIGHT` by 620 px (`NIGHT_CLEAR_R`, `NIGHT_DARK_R`, `NIGHT_STRENGTH` in
   `game/mod.rs`). It darkens the edges without hiding anything near the player.
8. `world.draw_overhead`: bats crossing the screen and fireflies over the ground,
   on top of the darkness.
9. Key prompts (E) over the flask or NPC in reach, drawn last so they're never hidden.
10. The item-get burst and banner, then the hearts, which are always shown whatever the
   HUD setting, then the skill bar (`skills::draw_bar`) at the bottom centre, hidden
   while the dialogue box (also bottom centre) is open and after death. If a battle is starting, the iris transition is drawn on top.

While `battle.scene_visible()`, the battle view replaces all of this
(`Battle::draw_scene`, then hearts, then the transition). Its backdrop
(`battle::draw_backdrop`) is a moonlit clearing: a starry sky, the moon, the manor
on a far hill with the same lightning, a treeline, and mossy stone platforms.

**Render targets.** The player and the monsters each draw into an off-screen canvas
(`OnceCell<RenderTarget>`, at 2× resolution), which is then blitted. The canvas lets
a sprite be rotated (roll), squashed (jump), or made translucent (flicker) as one
piece without seams. It's created on first draw.

**The art vocabulary** lives in `gfx.rs`: a deep plum `INK` for outlines instead of
black, a cool night `SHADOW`, the palette constants `MOON` (cool highlights and
rims), `LANTERN` (warm light), `LAB_GREEN` (anything the mad scientist made) and
`NIGHT`, the shared `shade`/`mix` maths, the vertex-coloured gradient meshes
(`gradient_ring`, `glow`, `vignette`), `potion_flask` (shared by the road pickup and
the item-get reward), and `hash01` for cheap deterministic noise. New art should use
these so the storybook-cartoon look stays consistent.

**Screenshots.** Translucent draws leave the framebuffer's alpha below 1. The window
ignores it, but the `JEKO_SHOT` hook forces alpha to 255 before saving, so a saved
shot matches the screen.

**Accessibility.** `Settings.reduce_flashing` swaps both flickers for a steady tint
and turns off the hit shake. The battle transition and item-get burst also honour it,
the manor's lightning becomes a steady pale glow (in both views), the fireflies stop
pulsing, and the battle sky's stars stop twinkling. Any new flash, blink or shake must
check it.

---

## 8. Battles and monsters (`game/battle.rs`, `game/monster.rs`)

Walking into an uncleared zone calls `Game::start_battle(kind, zone)`, which clears
the trigger, input and action, turns the player to face the monster on the right,
and sets `battle = Some(Battle::new(..))`. The battle
reuses the question pipeline (§5) for every question.

```
Enter (1.0 s: flash, iris closes on the player, opens on the battle)
  └─▶ Intro (0.8 s) ─▶ Question ◀───────────────────────────────┐
                         │ correct              │ wrong          │
                         ▼                      ▼                │
                 PlayerAttack (0.8 s)    MonsterAttack (0.9 s)   │
                 arrow hits at 0.4 s:    hit at 0.4 s:           │
                 hits_left −= 1          damage_player(damage),  │
                         │               stun, hat steal         │
                hits_left == 0? ──no─────────────┴── next question
                         │ yes
                         ▼
                 Faint (1.0 s) ─▶ Exit (0.7 s iris) ─▶ battle = None
```

*Figure: `battle::Phase`. Each move into `Question` requests a new task with the
monster's `difficulty_bias`.*

The player's attack is a bow shot. A correct answer calls `Player::shoot_at_monster`,
and the battle view (`battle::draw_shot`) flies an arrow from the bow at
`bow::RELEASE_S` (0.26 s) onto the monster at `PLAYER_HIT_AT` (0.4 s), with an
impact star. The player stays on their pad. The shot is drawing only: the hit is
counted by the phase timer, as before.

On victory the zone is marked `cleared`, a stolen hat comes back, `monsters_beaten`
goes up, and a toast shows. There's no fleeing. Dev battles (`JEKO_DEV=jelly@…`) use
zone index `usize::MAX`, so nothing gets cleared.

`MonsterKind::info()` holds all the gameplay numbers. `MonsterKind::pick` weights the
roster by distance: Pinchlings thin out, and the tough ones grow common further up.

| Monster | Hits | Damage (½♥) | Special | From | Bias | XP/hit |
|---|---:|---:|---|---:|---:|---:|
| Pinchling | 1 | 1 | none | 0 m | 0 | 15 |
| Jellyglow | 1 | 1 | stun 2.5 s | 30 m | 0 | 20 |
| Gull Bandit | 2 | 1 | steals the hat until beaten | 50 m | 0 | 20 |
| Puffspike | 2 | 1 + misses | grows with each wrong answer | 70 m | 1 | 20 |
| Coco Knight | 3 | 2 | stun 2.0 s | 140 m | 1 | 25 |
| Dune Wyrm | 3 | 2 | rare mini-boss | 400 m | 2 | 30 |
| Tide Titan | 4 | 2 | boss, only at each 500 m mark | 500 m | 2 | 40 |

---

## 9. Player, health and effects

- **Movement** (`Player::update`): WASD or the arrow keys at `base_speed` 140 px/s
  times `progression.speed_multiplier()`, times `SPRINT_MULTIPLIER` (1.6) while Shift
  is held, the player is moving and `Stamina::can_sprint`, then the `constrain` closure. The walk
  cycle advances by the distance actually moved, so it stops against the railing. A
  stun or the cheer pose blocks movement. `Player::stop(dt)` holds the player in place
  but keeps effects ticking.
- **Cosmetic actions.** `Action::Jump` (Space), `Action::Roll` (Ctrl) and
  `Action::Shoot` (F, `SHOOT_KEY`) change only how the sprite is drawn. They never
  change position, speed or anything gameplay reads. Only one runs at a time.
- **Skills** (`game/skills.rs`, FR26). `Player.skills` limits the roll and the sprint.
  The roll has `ROLL_CHARGES` (3): Ctrl spends one (`RollCharges::try_use`) and does
  nothing with none left. A spent charge comes back after `ROLL_RECHARGE_S` (1.0 s)
  without rolling; spending the last starts a `ROLL_LOCKOUT_S` (1.5 s) lockout that
  then refills all 3. Stamina is 0..1: sprinting drains it in `SPRINT_S` (3 s); it
  refills from empty in `STAMINA_REFILL_S` (4 s), starting `STAMINA_REGEN_DELAY_S`
  (0.5 s) after the sprint stops. Running it dry leaves the player winded (no sprint,
  walking unaffected) until it's back to `WINDED_UNTIL` (35%). `Skills::update` runs
  once a frame from `Player::update` or `Player::stop`, so the skills recover at a
  flask, in a battle and while stunned, and freeze on pause. `Player::new` resets them.
- **The bow** (`game/bow.rs`). It rides slung across the back. A shot lasts `SHOOT_S`
  (0.55 s): raise, draw (the far eye squints), release at `RELEASE_S`, follow through,
  lower. Facing is locked while it plays. On the road the release looses an `Arrow`
  into `Player.arrows` (at most `MAX_ARROWS`, 8). It flies across at 560 px/s, comes
  down under gravity about 340 px away, then sticks in the road or ground for 1.4 s, or
  splashes if it lands west of `bank_x`, in the lake. Arrows tick with the player's effects, so
  they freeze with a pause. Nothing collides with them.
- **Status.** Stun (`stun(secs)` extends a running one) shows a halo of stars, blocks
  movement and blocks submitting an answer. The cheer is the item-get pose. `dead`
  drives the flop-and-fade. `hat_stolen` is set by the Gull Bandit.
- **Flicker** (`game/flicker.rs`). The spawn `Ghost` lasts 2 s and doubles as spawn
  protection. The hurt `Damage` blink lasts 1 s. Either one being active is what makes
  `damage_player` ignore hits.
- **Health** (`game/health.rs`). 5 hearts, stored as 10 half-hearts and clamped at both
  ends. Heals: ½ heart for a correct flask, a full refill on a level-up. Death sets
  `game_over = Some(0.0)`, clears pause and settings, records `best_distance_m`, and
  marks any open flask `Missed`.

---

## 10. Progression and the UI layer

**Progression** (`progression/`) is plain logic with no rendering:

- XP fills toward `xp_to_next_level`. It starts at 100 and grows ×1.3 per level.
  Level-ups loop, so one big gain can cross several levels.
- Streak multiplier: `1 + 0.1 · min(streak, 10)`, so up to +100%. A wrong answer resets
  the streak to 0. This is the main "one more" hook.
- Upgrades unlock from `lifetime_xp`, not from a shop. `record_correct` returns the
  ones it just unlocked, for the toast. `speed_multiplier()` is the max over unlocked
  upgrades. Today there's one: Quickstep Boots, +25% at 100 XP. To add an upgrade, add
  an entry to `default_upgrades()` and, for a new kind, its effect. The unlock logic is generic.
- Difficulty is `progression.level + bias`, capped at 5 (§5).

**The UI** (`ui/`) is immediate-mode egui, rebuilt every frame from `Game`. A panel
owns no state: it reads what it's given, and the only things it changes are the
values passed to it as `&mut`.

| Panel | Anchor | Reads | Writes / returns |
|---|---|---|---|
| `hud::draw` | top right | `Progression`, `distance_m` | none |
| `hud::draw_toast` | | `game.toast` | none |
| `task_panel::draw` | left centre (bottom centre in a battle) | `TriggerState` | `input_buffer`, `focus_answer`; returns `Submit` |
| `token_panel::draw` | bottom right | `CerebrasClient::token_stats()`, model, base URL | none |
| `dialogue_panel::draw` | | `Dialogue` | none |
| `settings_panel::draw` | modal | | `Settings`, `settings_open` |
| `game_over_panel::draw` | modal | `Game` (run stats, best distance) | returns `true` to restart |

`ui::style::apply` sets the theme every frame: pale parchment panels (kept light so
text reads against the dark world), thick dark-wood borders, hard offset shadows, a
lantern-amber hover and a moss-green accent. `gfx::key_prompt` copies the palette, so world
prompts match the panels. `Settings` (`show_hud`, `show_token_panel`,
`reduce_flashing`) lives on `Game` and isn't saved between launches.

---

## 11. Content, configuration and dev hooks

**Assets** are compiled into the binary with `include_str!`, so editing them needs a
rebuild:

| File | Loaded by | Used for |
|---|---|---|
| `assets/tasks_fallback.json` | `main.rs::load_fallback_tasks` | Questions when there's no key or a call fails. Must deserialize as `Vec<MathTask>` and be non-empty. |
| `assets/dialogue.json` | `dialogue::load_bank` | NPC lines. Professor Pebble says the hub tutorial and then rotates through their bank lines. Regenerate it with `make gen-content`. |

**Configuration.** `CEREBRAS_API_KEY` is optional; without it the game runs offline.
`CEREBRAS_MODEL` defaults to `kimi-k3.1`, a placeholder slug.
`CEREBRAS_BASE_URL` defaults to `https://api.cerebras.ai/v1`. Values come from the
environment, a `.env` file, or secretspec (`secretspec.toml`). See the README.
`JEKO_INSTANCE` names the checkout for the window title (`JEKO — Math Quest [name]`)
and the startup stderr line. It defaults to the build folder's name, so two clones
run side by side can be told apart. Nothing else is shared between clones: each has
its own `target/` and `.env`, and the game opens no ports and writes no files apart
from a `JEKO_SHOT` screenshot.

**Dev hooks** (`main.rs` plus `Game::dev_event`) let a change be checked without
driving the window:

- `JEKO_SHOT=out.png:120` saves a screenshot at frame 120, prints
  `Game::debug_summary()` (a `STATE …` line) to stderr, and exits.
- `JEKO_DEV=name@frame,…` fires scripted events: damage (`hurt`, `kill`), moves
  (`jump`, `roll`, `shoot`, `stun`, `cheer`), `spent` (empty the roll charges and
  stamina, to show the skill bar recovering), `restart`, `pause`, `settings`, battles
  (`pinch`, `jelly`, `puff`, `coco`, `gull`, `wyrm`, `titan`), answers through the
  real submit path (`right`, `wrong`), `e` (press E), teleports (`toflask`,
  `tozone`, `topebble`), and display settings: `nohud` (hide both side panels, so
  a shot shows the whole world) and `calm` (turn on reduce flashing).
- With either variable set, the loop steps a fixed 1/60 s, so a run is repeatable
  frame by frame. Without an API key, questions come straight from the fallback
  bank, so `right`/`wrong` work on the next frame.

**Tests** are unit tests inside the modules. Run them with `make test`, which runs
`cargo test` in the nix dev shell.

| Module | What's covered |
|---|---|
| `game/world.rs` (10) | Scenery never lands on the road. Every flask is reachable and on the road. Generation is deterministic. Interact reaches only nearby fresh flasks. The clamp keeps the player on the road. Encounters are on the road and deterministic. A Titan guards every 500 m. Tougher monsters appear further up. The hub is wide, dry and bounded. The walkable width is continuous through the gate. |
| `game/manor.rs` (1) | Lightning strikes now and then (under 12% of frames), stays in 0..1, and is the same every run. |
| `llm/usage.rs` (5) | Provider-timed vs round-trip speed, zero time ignored, no usage block, the capped history. |
| `game/bow.rs` (2) | The shot draws, then releases, then lowers. An arrow flies the way it was loosed, comes down a believable distance away, then goes away. |
| `game/health.rs` (4) | Starts full; damage, heal and refill clamping. |
| `game/skills.rs` (5) | Three rolls then a lockout that refills all; a spent charge trickles back; sprinting drains and running dry winds; stamina waits, then recovers, and winded ends at the threshold; a short break doesn't refill. |
| `game/dialogue.rs` (2) | The bank has Professor Pebble; the typewriter reveals, then advances, then closes. |
| `ui/token_panel.rs` (1) | Token counts read `1,284` below 10,000, then `12.9K` / `4.2M`. |

**Build environment.** `flake.nix` provides the dev shell (rustc, cargo, and the
X11/GL/xkbcommon/ALSA libraries on `LD_LIBRARY_PATH`) and a separate `openclaw`
shell. `make` targets wrap `nix develop --command`; `make info` prints which checkout,
branch and `.env` the current folder uses. `egui-macroquad` is pinned to
0.17 because 0.15 pulls in macroquad 0.3 alongside 0.4 and fails to link (duplicate
`CONTEXT`). Audio is out of scope, and the ALSA startup panic has a null-device
workaround. The README has details on both.

---

## 12. Invariants: what a change must keep

These are the load-bearing decisions. Breaking one is a design change: raise it with
the owner, and update this section if it goes ahead.

1. **The frame never waits on the network.** Every model call goes through a
   background thread and an `mpsc` channel that's polled with `try_recv`. Never
   `.await` or block on a request inside `update`/`draw`.
2. **Every question path works offline.** No key, a timeout, an HTTP error and a bad
   payload all fall back to `assets/tasks_fallback.json`. A new question source needs
   the same fallback.
3. **The world is a pure function of `y` and the chunk index.** Generation uses only
   seeded `StdRng` or `gfx::hash01`, never `thread_rng`, so it's the same every run and
   the placement tests stay meaningful. New placed things (blockers, cages,
   shells) should follow `spot_for_chunk` / `encounter_for_chunk` and get a
   determinism test.
4. **The hub is a safe zone.** Nothing hostile spawns south of `ROAD_SOUTH_END`
   (chunk `k < 1`).
5. **`Game` owns run state, and panels are views.** UI functions take what they need
   and return events. They don't keep gameplay state of their own.
6. **One door per rule.** HP changes go through `damage_player`, `Health::heal` and
   `Health::refill`. Questions start through `request_task`. Answers are judged in
   `submit_answer`.
7. **Cosmetic means cosmetic.** `Action`s, the bow's arrows, the planned pet (FR4)
   and visual effects never change XP, speed, answers or battle results.
8. **Animation runs on the game clock.** Use `Game.time` or per-object timers advanced
   by `dt`, so pausing freezes everything.
9. **Reduce flashing is honoured** by every flicker, flash and shake.
10. **Layering** (§2): `config`/`llm`/`progression` stay free of macroquad and egui,
    `game` stays free of egui.

---

## 13. Known limits and open design

These are known gaps, not bugs. Each one is either filed as an FR or a decision
still to make:

- **No persistence.** Progress, settings and best distance live only in memory (FR14
  and FR16 record this as provisional).
- **Answers are numeric only.** `MathTask.answer` is an `f64` with an exact-match
  check. Non-math subjects or multiple choice (an FR3 open question) would need a
  new answer type through the prompt, the parser, `task_panel` and `submit_answer`.
- **One thread and one HTTP client per request**, with no cancellation or retry. That's
  fine at one request at a time; it should be revisited if requests ever overlap or prefetch.
- **No curse or road blockers yet** (FR1, FR2). Encounter zones are the only hostile
  placement, and nothing pushes the player forward.
- **The difficulty cap is 5** and comes from level plus monster bias only. It doesn't
  rise with distance.
- **The hero and the monsters still dress for the beach.** FR24 changed the world
  but not the characters: the hero wears a straw sun hat and swim shorts, and the
  roster is crabs, jellyfish, pufferfish, coconuts, gulls, a sand worm and a Tide
  Titan. Restyling them is FR25.
- **The settled-for-now defaults** from FR5–FR15, FR20 and FR24 are listed for the owner in FR16.

See [FeatureRequestToDo.md](../FeatureRequestToDo.md) for the backlog.
