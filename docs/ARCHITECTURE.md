---
type: Reference
title: JEKO — Architecture
description: As-built overview of JEKO's architecture — crate layering, the frame loop, the Game state owner and its modes, the live-question pipeline to Cerebras, procedural world generation, 2.5D rendering, battles, data-defined monsters and hot reload, progression, the egui layer — and the invariants a change has to keep.
tags: [architecture, design, overview, rust, macroquad, egui, cerebras]
timestamp: 2026-11-21
---

# JEKO — Architecture

> **Bottom line.** JEKO is one Rust crate: a library plus three binaries. A single
> macroquad loop in `src/main.rs` drives one owner of all run state, `game::Game`,
> which updates and paints the world each frame. egui panels are drawn on top as
> thin views of that state. Only one thing is slow: asking Cerebras for a math problem.
> That call runs on a throwaway OS thread and its result comes back over an `mpsc` channel
> that `TriggerState` polls each frame. The game asks ahead, too: it looks up the road for flasks
> and monsters and keeps their questions ready in a buffer (FR32), so most questions are on screen
> the frame they're needed. When there's no key or the call fails, a fallback
> bank compiled into the binary steps in. The world isn't stored as a map. The path is a function of `y`, and each chunk's
> scenery, potion flask and monster (hiding in ferns, or blocking the road) is a pure
> function of the chunk index, so the road is endless, the same every run, and unit-testable. The setting is a dark forest
> at night (FR24), with the mad scientist's manor drawn as a backdrop across the lake.
> Besides the hand-drawn monsters, monsters can be defined as JSON specs in a content
> dir (FR21). A watcher thread reloads them while the game runs, over the same kind
> of polled channel, and a studio thread designs new ones with the model in the
> background and writes them to that dir, lowest priority, never in a question's way. The same
> studio writes lesson packs ahead of need: every generated problem's answer is checked against
> its own arithmetic before a child sees it (FR22).
> Once the player steps onto the road, the doctor's curse (FR1) worsens while they
> linger, which is what keeps them moving.

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
                                                │ error, no API key, or past the soft deadline
                                                └──▶ random task from assets/tasks_fallback.json

Game::tick_prefetch ── world.upcoming(1.5 screens) ─▶ plan_prefetch ─▶ prefetch_task_async
  └─ answers land in prefetch.buffer (QuestionBuffer); request_task takes from it first

content::watcher thread ── rescans <content dir>/monsters, lessons and tips (*.json) every 0.5 s
  └─ ContentEvent over mpsc ── Game::poll_content, try_recv() each frame ──▶ Game.roster

content::studio thread ── waits for a job and for the model to be free (Game::tick_studio)
  └─ prompt ─▶ model ─▶ finish_monster (tier forced, special limited, text vetted, parse)
       │                 finish_pack (every answer checked against its expression) · tips::finish
       └─ writes <content dir>/monsters/gen-<name>.json · lessons/<topic>-<level>.json · tips/<topic>.json
            └─▶ the watcher above loads them: specs to the roster, packs to the question buffer, tips to Pebble
```

*Figure: the frame loop and its two background paths: questions from Cerebras (asked
for live or ahead of need), and monster specs from the content dir.*

---

## 2. Crate layout and layering

One Cargo package, `jeko` (`Cargo.toml`, edition 2021):

| Target | Source | Role |
|---|---|---|
| library `jeko` | `src/lib.rs` | `config`, `content`, `llm`, `game`, `progression`, `ui`. It exists so both binaries share `CerebrasClient`. |
| binary `jeko` (default-run) | `src/main.rs` | The game: window config, the frame loop, the dev hooks (§11). |
| binary `gen_content` | `src/bin/gen_content.rs` | Offline tool. Asks the model for NPC lines and writes `assets/dialogue.json` once `dialogue::check_bank` accepts the whole bank, and designs monsters into the content dir. The game never runs it. |
| binary `validate_content` | `src/bin/validate_content.rs` | Offline checker, no key and no writes. Runs `dialogue::check_bank` on the dialogue bank and the watcher's own scan on the content dir, prints each rejection and exits non-zero if there is one (`make validate`). It is how a person or an OpenClaw agent tests what it wrote before the game sees it. |

Which module uses which (arrows mean "uses"):

```
main.rs ──▶ ui ──▶ game ──▶ llm ──▶ config
  │         │        ├────▶ progression
  │         │        └────▶ content
  │         └──────────────▶ progression, llm   (read-only, for the HUD and token panel)
  └──▶ content   (starts the watcher and the studio)
gen_content.rs ──▶ llm, config, content, game::dialogue (check_bank)
validate_content.rs ──▶ content, game::dialogue (check_bank)
```

The layering holds today, and new code should keep it:

- **`config`, `content`, `llm`, `progression`** know nothing about macroquad or egui. They're
  plain Rust, so they can be tested and reused without a window (`gen_content` uses
  `llm` this way). The studio thread lives in `content` and uses `llm` for the call.
- **`game`** uses macroquad (input, drawing, render targets) and never egui. It
  mentions egui only in comments.
- **`ui`** is the only module that imports egui. Its panels are functions, not
  stateful widgets (§9).

| Module | Owns |
|---|---|
| `config.rs` | `Config::load()`: reads `CEREBRAS_API_KEY`, `CEREBRAS_MODEL`, `CEREBRAS_BASE_URL` from the environment, after `dotenvy` loads `.env`. An empty key counts as no key. `instance_name()`: `JEKO_INSTANCE`, else the folder the binary was built from (`CARGO_MANIFEST_DIR`), shown in the window title. |
| `content/mod.rs` | `ContentEvent` (a spec, lesson pack or tips file loaded or reloaded, a monster removed, a file rejected), `Roster` (the loaded specs by id, sorted), and `content_dir()`. |
| `content/monster_spec.rs` | `MonsterSpec`, a monster as data, and `parse`, the only way in: JSON in, a clamped and validated spec out. `MonsterInfo`, the gameplay numbers every monster has, and the tier table (`MonsterSpec::info`). |
| `content/studio.rs` | `Studio`, the background designer and writer: one thread, a job queue (`Job::Monster`, `Job::Lessons`, `Job::Tips`), the gate that keeps it behind live questions (`set_live_busy`), the per-minute cap and token budget (`StudioConfig`), and `StudioStatus`. `finish_monster` turns a model reply into a spec file (the only writer of generated specs), `vet_text` is the word blocklist, and `fake_monster_json` is the stand-in model's creature. |
| `content/prompts.rs` | `monster_prompt`: what the studio asks the model for (a kid-safe, original creature in the spec's own vocabulary, with a shipped spec as the shape to follow), the lists of allowed values, and the maths topics. |
| `content/lessons.rs` | Lesson packs (FR22): `parse_pack` (verifies every problem when a file loads), `finish_pack` (a model reply to a verified pack file, never overwriting), and the stand-in model's pack. |
| `content/tips.rs` | Professor Pebble's tips (FR22): the prompt, `parse` and `finish`. |
| `content/watcher.rs` | `Scanner` (one rescan of `monsters/`, `lessons/` and `tips/`, by modification time and length) and `spawn`, the thread that runs it every `POLL` (0.5 s). |
| `llm/mod.rs` | `MathTask { topic, problem, answer: f64, hint, difficulty, explanation, expression }`, the one task type every question path produces (the last two are FR22's and default to empty), and `fallback_bank()`, the compiled-in bank. |
| `llm/expr.rs` | `eval` (a tiny arithmetic evaluator) and `verify` (a generated problem passes only if its `answer` equals its `expression`; FR22). |
| `llm/activity.rs` | `GenActivity` (FR33): the jobs the creation bar shows, one per model call being written, each `Writing`, then `Ready` or `Fallback`. `view()` gives the bar's circles (oldest three, and the count of the rest). A read-only mirror: nothing reads it to decide anything. |
| `llm/buffer.rs` | `QuestionBuffer` (ready questions per difficulty, with an optional strict topic), `plan_prefetch` (the pure look-ahead policy and its `PrefetchCaps`), and `WaitStats` (where each shown question came from and how long the player waited). |
| `llm/cerebras_client.rs` | `CerebrasClient`: the async wrappers (live and look-ahead), the blocking chat-completions call with its `RequestKind` tag, the shared token stats, and the `fakemodel` stand-in. |
| `llm/task_gen.rs` | The prompt templates (`build_prompt(difficulty, topic)` for a live question, `build_pack_prompt` for a lesson pack) and lenient JSON extraction (`extract_json_object` / `_array`). |
| `llm/usage.rs` | `RequestKind` (question, prefetch, monster, lesson), `RequestUsage` (one call's tokens and speed) and `TokenStats` (session totals, per-kind totals, and the last 24 requests). |
| `game/mod.rs` | `Game`: run state, the per-frame update and draw, the mode precedence, the damage, answer and reset entry points, and the dev events. |
| `game/task_trigger.rs` | `TriggerState`, the question state machine (§4), `Arrival` (how a `Loading` request ended), and `pick_fallback`. |
| `game/prefetch.rs` | `Prefetch`: the question buffer, the look-ahead requests in flight and their per-minute budget; `demand` (what the road ahead needs); `plan_lesson` (which pack the studio writes next); the soft deadlines; `Ask` (a question asked for and not yet shown). |
| `game/field_guide.rs` | `FieldGuide` (the cards unlocked this session) and `GuideCard`. Pure data; `ui/field_guide_panel.rs` draws it. |
| `game/world.rs` | Coordinates and camera (`View`), path geometry, chunking, potion flasks (`Spot`), encounter zones and road blockers (`EncounterZone`, `clamp_blocked`), the goalposts the tracker counts down to (`Goalpost`, `goalposts_in_chunk`, `pick_goalpost`, `World::next_goalpost`), what will need a question soon (`Upcoming`, `upcoming_in`, `World::upcoming`), the depth-sorted `Tall` items, and drawing the ground, the cobbled road, the light pools, the mist, the bats and fireflies, and where the manor sits on screen. |
| `game/decor.rs` | Per-chunk forest scenery generation (`generate_chunk`), the hand-placed hub camp (`hub_layout`), how each prop draws, and which props cast light (`Decoration::lights`). |
| `game/manor.rs` | The gothic manor backdrop (`draw`, with an optional lake reflection), its lightning timing (`lightning`), and the shared `bat` silhouette. |
| `game/gfx.rs` | The shared drawing vocabulary: the `INK`, `SHADOW`, `MOON`, `LANTERN`, `LAB_GREEN` and `NIGHT` colours, shading, ellipses, ribbons, stars, gradient meshes (`gradient_ring`, `glow`, `vignette`), the potion flask, hearts, the key prompt, the item-get burst, and `hash01`. |
| `game/player.rs` | `Player`: movement, the cosmetic `Action`s (jump, roll, shoot), the arrows it has loosed, its `Skills`, stun, cheer, death, the stolen hat, and the sprite drawn through a render target. |
| `game/skills.rs` | Movement skills (FR26): `RollCharges` (3 rolls, a trickle, a lockout) and `Stamina` (sprint drain, regen, winded), held together in `Skills`, and `draw_bar`, the skill bar at the bottom centre. |
| `game/bow.rs` | The bow: the shot's timing (`aim`, `SHOOT_S`, `RELEASE_S`), drawing the bow slung and held and an arrow (`draw_arrow`), and `Arrow`, a cosmetic projectile that arcs, then sticks in the ground or splashes in the lake. |
| `game/map.rs` | The discovery map (FR27): `Discovery` (the furthest `y` reached, kept on `Game`) and `snapshot`, which samples the world functions and the flasks and zones for the discovered stretch and returns a `MapView` (landmarks, the road's centreline, the player). No egui. |
| `game/flicker.rs` | `Flicker` / `FlickerKind` (spawn ghost, damage blink). It turns elapsed time into a `Look` (tint and alpha) and honours reduce flashing. |
| `game/goalpost.rs` | `Tracker` (FR31): which goalpost the top-centre badge shows, the metres left, and its pop, unroll, "Now!" and close animation. It produces a `TrackerView` and draws nothing. |
| `game/health.rs` | `Health`, counted in half-hearts: damage, heal, refill, clamped. |
| `game/curse.rs` | `Curse` (FR1): the curse level and its rules (`tick` returns a `Tick` for `Game` to act on, `cleanse`, `speed_factor`), its `Stage`s, and its drawing (`draw_tint`, `draw_wisps`, `draw_meter`). |
| `game/monster.rs` | `Species` (a built-in `MonsterKind` or a `MonsterSpec`), the `MonsterKind` roster and its stats, distance-weighted `pick`, the shared canvas and poses, and per-monster drawing and animation. |
| `game/monster_rig.rs` | `draw_spec`: draws any `MonsterSpec` from its body shape, parts, eyes, accessory, idle and attack. |
| `game/battle.rs` | `Battle` and its `Phase`s, the iris transition, and the battle-view drawing. |
| `game/dialogue.rs` | `Dialogue` (typewriter reveal), the `PEBBLE_TUTORIAL` lines, and `load_bank()` for `assets/dialogue.json`. |
| `progression/` | `Progression` (XP, level curve, streak multiplier, upgrade unlocks, and FR22's per-topic accuracy and the problems missed) and `upgrades.rs` (upgrade definitions). |
| `ui/` | One egui function per panel (`hud`, `goalpost_panel`, `creation_bar`, `task_panel`, `token_panel`, `settings_panel`, `field_guide_panel`, `map_panel`, `minimap`, `dialogue_panel`, `game_over_panel`), plus `style::apply`. |

---

## 3. The frame loop (`src/main.rs`)

Before the loop, `main` builds `Game`, starts the content watcher on
`content::content_dir()` (`Game::watch_content`) and, when there's a key (or the
stand-in model), the studio (`Game::attach_studio`). macroquad's `#[macroquad::main]` gives an `async fn main`, but `next_frame().await`
only yields once per frame. **It isn't an executor that can drive other futures**,
which is why network work goes to a thread (§4). Each iteration does this, in order:

1. `clear_background` (`gfx::NIGHT`).
2. Fire any `JEKO_DEV` events scheduled for this frame (`Game::dev_event`).
3. Choose `dt`: a fixed 1/60 s when `JEKO_SHOT` or `JEKO_DEV` is set, so frame numbers
   map to game time; `get_frame_time()` otherwise.
4. `game.update(dt)`: content events first (§8), then input, simulation, polling the
   questions and starting look-ahead requests (§5).
5. `game.draw()`: the world or the battle view, in screen space (§7).
6. `game.draw_pause_overlay()`: the pause dim, or the dim behind Game Over.
7. **egui pass** (`egui_macroquad::ui`): `ui::style::apply`, then set
   `game.typing = ctx.wants_keyboard_input()`, then draw the panels. The HUD and token
   panel are hidden during a battle, a dialogue or while the map is open. The goalpost badge (`goalpost_panel`, FR31) shows when
   `game.goalpost.view()` has something, and the toast steps down below it. The creation bar (`creation_bar`, FR33, `settings.show_creation_bar`) sits under the badge and pushes the toast down in turn; it hides in the hub, in a dialogue, on the map and at Game Over, and in a battle it holds only the battle's own question. The map panel (`map_panel::draw`) draws from `game.map_view()`. The minimap (`minimap::draw`, top right, `settings.show_minimap`) draws the same `MapView` as a small window around the player, hidden in the same cases as the HUD, and returns its bottom edge so the HUD sits beneath it. The task panel returns
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
`world`, `progression`, `health`, `curse`, `trigger_state`, `battle`, `dialogue`, `settings`,
`run` stats, the camera (`cam_y`, `shake`), toasts, the goalpost `Tracker`, the game clock `time`, and the
spec monster `roster` with the watcher's channel, the `studio` handle, and the question `prefetch` buffer with its `waits` readout. The UI
gets it by reference. Nothing else holds gameplay state.

`Game::update` runs these checks in a fixed order. The first one that matches decides
what the frame does:

| # | Mode | Condition | What the frame does |
|---|---|---|---|
| 0 | (every frame) | | `poll_content` drains the content watcher into `roster` (§8). It runs before everything else, so an edit shows while paused or after death. |
| 1 | **Game over** | `game_over.is_some()` | Only the death animation advances. After `DEATH_ANIM_S` (1.2 s), E or Enter calls `reset()`. |
| 2 | Toggles | not `typing` | With the map open, M or Esc closes it. Otherwise M opens it (`can_open_map`: on the road, no battle, dialogue, open question, settings or field guide). O toggles `settings_open`, and Esc closes it. With settings closed, G toggles the field guide (`guide_open`, Esc closes it) and P toggles `paused`. |
| 3 | **Paused** | `paused \|\| settings_open \|\| guide_open \|\| map_open` | Only `poll_questions(0.0)` runs (the shown question, the battle's next one and the look-ahead receivers), so a response that arrives isn't lost, and the studio's gate is set to busy. The clock doesn't advance, and neither does the soft deadline. |
| 4 | (every unpaused frame) | | Advance `time`, shake, the toast, the explanation card and the item-get timers, then `tick_goalpost` (the tracker, §10) and `tick_studio` (§8), which also runs in battles and dialogues. |
| 5 | **Battle** | `battle.is_some()` | `update_battle`: the player is held, phases advance (§8), and each question is asked for early and shown when the phase reaches `Question` (§5). |
| 6 | **Dialogue** | `dialogue.is_some()` | The player is held. E reveals the line, then advances, then closes. |
| 7 | **Road** | otherwise | See below. |

The road frame, in order:

1. **Busy check.** While `trigger_state` is `Loading` or `Active`, the player is held
   (`player.stop`). Each flask is a checkpoint, so a fast model means a short stop.
2. Otherwise `player.update(dt, progression.speed_multiplier() × curse.speed_factor(),
   constrain)`, where `constrain` is `world::clamp_to_path` followed by
   `World::clamp_blocked` (an undefeated road blocker shuts the road, §6), then
   `tick_curse(dt)` (§9). If the curse's drain killed
   the player, the frame ends here. The curse is ticked only on this branch, so it
   holds still while a question is loading or open, in a battle, in a dialogue and
   while paused.
3. `discovery.note(player.y)` records the furthest point reached (FR27). The camera eases toward the player (anchor 0.66 of screen height, rate `1 − e^(−6·dt)`).
   `world.ensure_generated` keeps one chunk past the top of the screen generated.
4. `poll_questions(dt)`, then `tick_prefetch` (§5).
5. If not busy, stepping into an uncleared fern patch starts a battle.
6. Interaction targets: the nearest fresh flask in reach (`interact_target`), or else
   an undefeated road blocker in reach (`blocker_target`), or else Professor Pebble.
   E opens the flask, challenges the blocker (`challenge_blocker`, which starts its
   battle), talks to Pebble, or focuses the answer field when a task is open.
7. A `Resolved` cooldown counts down to `Idle`.

**Single entry points.** Several systems have exactly one door, so there's one place
to change each rule:

- `Game::damage_player(halves, from)` is the only way HP goes down. It ignores hits
  while dead or while a flicker is active, which is the invulnerability window. It
  starts the damage flicker (`HURT_INVULN_S` = 1 s), knocks the player back 22 px away
  from `from` (clamped to the path and short of any road blocker), shakes the screen unless reduce flashing is on,
  and calls `die()` at 0 HP.
- `Game::request_task(bias, deadline_s, topic)` is the only way a question starts (§5). Look-ahead
  only fills the buffer it reads from.
- `Game::submit_answer()` is the only way an answer is judged, for flasks and battles alike.
- `Game::reset()` starts a new run without relaunching. It keeps the client, the
  fallback bank, the settings, `best_distance_m`, the question `prefetch` buffer and the content `roster` with its watcher. Everything else is rebuilt (the
  curse goes back to dormant). Requests in flight aren't thrown away: `stash_questions`
  hands their receivers to `prefetch`, so the answers still reach the buffer.

**The game clock.** `Game.time` advances only on unpaused frames, and every animation
reads it. Lanterns, flasks, lightning, mist and monsters therefore freeze with a pause.
Animations must use it, not wall time.

---

## 5. The question pipeline (flasks and battles)

The game's reason to exist is that a fresh, difficulty-scaled problem shows up while
the player is still moving. Potion flasks and battles share one path:

```
   open_spot / battle start / battle answer wants a question
                              │
                Game::request_task(bias, deadline_s)
                difficulty = min(level + bias, 5)
        ┌─────────────────────┼──────────────────────┐
  buffer has one         API key set             no API key
  (this level, else           │                       │
   one below)       Loading(request_task_async)  Active(pick_fallback)
        │                     │
  Active(task)                │ poll() each frame; past the soft deadline: Active(pick_fallback),
                              │ and the late answer goes into the buffer
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

**Asking ahead (FR32).** The waits come from timing, not from the model being slow, so
the game asks before the player needs the answer:

- **Battles overlap their own animations.** The first question is requested in
  `start_battle` and held in `Game.next_question`. It's shown when the intro ends, so
  the entry transition and the intro (1.8 s) cover the round trip. Each later question
  is requested in `battle_answer`, when the answer is submitted, so the 0.8 s player
  attack or 0.9 s monster attack covers it. The answer that lands the last hit asks for
  nothing. Both are shown when the phase next reaches `Question`.
- **Road look-ahead.** Each road frame `tick_prefetch` asks `World::upcoming` for the flasks
  and monsters up to `LOOKAHEAD_SCREENS` (1.5) screen heights north of the player.
  `prefetch::demand` turns them into wants (a flask: one question at the player's level; a
  monster: its `hits` plus a spare, at level + its difficulty bias), and the pure
  `llm::buffer::plan_prefetch` decides what to request under the caps: 2 in flight, 3
  ready or in flight per difficulty, `JEKO_PREFETCH_RPM` (20) per minute. Answers land in
  `Prefetch.buffer` (a `QuestionBuffer`). `JEKO_PREFETCH=off` turns the look-ahead off.
  `request_task` takes from the buffer first: the exact difficulty, else one below (after a
  level-up), else it asks live. The buffer survives `reset()`.
- **A live request beats a look-ahead.** `tick_prefetch` starts nothing while the shown
  question or the battle's next one is `Loading`, while paused, or in a battle. Receivers
  are still polled while paused.
- **The soft deadline.** A live question that hasn't landed after `SOFT_DEADLINE_BATTLE_S`
  (1.5 s) or `SOFT_DEADLINE_FLASK_S` (2.5 s) of unpaused game time is replaced by a
  fallback-bank question. The late answer isn't dropped: its receiver is adopted by
  `Prefetch` and goes into the buffer. The 8 s HTTP timeout remains the backstop. The same
  adoption saves the answers of requests that a new battle, the end of a battle or a reset
  abandons (`stash_questions`).
- **Themed battles** (FR22). A spec monster's `theme.topic` (when its subject is maths)
  goes through `Species::topic()` into `request_task`. With a topic, `QuestionBuffer::take`
  takes only a question on that topic (lesson packs are what fill it), a live call puts it in
  the prompt and forces it onto the task, and the offline bank prefers it. A fraction-loving
  jellyfish asks about fractions; the built-ins ask about anything.
- **The wait is measured.** `Game.waits` (`llm::buffer::WaitStats`) records where each shown
  question came from (`Buffer`, `Ready`, `Live`, `Fallback`, `Deadline`) and how long the
  player watched the spinner. Offline play records nothing. The token panel shows "ready
  on arrival 18/20 · longest wait 0.3 s" and the request counts per `RequestKind`.

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
  with a `usage` block is recorded, tagged with its `RequestKind` (the panel splits the
  session's requests and tokens by it). Tokens per second come from Cerebras'
  `time_info.completion_time` when present, and from the HTTP round trip otherwise.
  The round trip reads lower, and the panel says which one it used. A transport or
  HTTP error is counted with `record_failure`. A poisoned lock is ignored, because the
  numbers only feed a readout.

**The fallback bank.** `assets/tasks_fallback.json` is compiled in with
`include_str!` (`llm::fallback_bank`) and parsed at startup, where a malformed file panics.
`pick_fallback` panics if the bank is empty.
`pick_fallback` takes a random entry (`rand::thread_rng`, the only non-deterministic
randomness in the game). The game must stay fully playable with no key and no network.

**Checked before a child sees it** (FR22). The live prompt also asks for the sum behind the
answer (`expression`, numbers and `+ - * /` only) and 2 to 4 `explanation` steps.
`llm::expr::verify` evaluates the expression with a small pure evaluator and rejects the
problem if the result isn't the `answer`; the call then fails like any other and the
fallback bank steps in. The same check runs on every prefetch and on every problem in a lesson
pack, where a missing or unparseable expression is also a rejection. The compiled-in bank
has expressions and explanations too, and a test verifies all of it.

**Lesson packs and practice** (FR22). The studio writes `lessons/<topic>-<level>.json`
(`Job::Lessons`, 8 problems; `Game::tick_studio` asks when fewer than 3 questions are ready
or coming at the player's level, `plan_lesson`). The watcher loads each pack's verified
problems into the question buffer (`push_unique`, so a reload doesn't double them), where
`request_task` takes them like any prefetched question. `Progression.topics` counts
answers per topic this run (`record_topic`, called from `submit_answer`), `weakest_topic()` is
the topic below 75% with at least 2 tries, and `plan_lesson` aims the next pack at it. A wrong
answer is remembered (`missed`, the last four) and comes back as a small pack of similar
problems (`like`). A topic that stays weak also gets one tips job for Professor Pebble
(`tips/<topic>.json`), whose lines Pebble says in the hub before the usual ones.

**Explanations.** A wrong answer with `explanation` steps sets `Game.explain`, a card
(`task_panel::draw_explanation`) beside the next question for 3 s plus 2 s a step, on the game
clock. The bank's questions have steps, so it works offline.

**Answers and their effects** (`Game::submit_answer`):

| | Correct | Wrong |
|---|---|---|
| **Flask** | `record_correct(20)` (times the streak multiplier). Heals ½ heart, or refills on a level-up. Cleanses 0.25 of the curse. Cheer pose and a "+N XP!" banner. The spot is marked `Solved`. | `record_incorrect()` resets the streak. `damage_player(2, None)` costs 1 heart. The spot is marked `Missed`. |
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
hovering over rune stones. Most monsters hide in patches of ferns; now and then one
stands in the open on the road and blocks it (FR2).

**Geometry as functions of `y`.** Nothing is a tiled map:

- `path_center(y)`: two slow sines, ramped in from dead straight over the first
  700 px, with a maximum slope of about 0.25 so the fence nudges rather than shoves.
- `bank_x(y)` (where the lake meets dry land) and `treeline_x(y)` (where the woods
  start) follow the path with their own wobble, so the ground never pinches shut.
- `walkable_x(y)` is the road's width (`PATH_HALF_W − WALK_MARGIN`) north of the
  gate. South of it, the width eases out over a 150 px funnel into the hub's open ground.
- `clamp_to_path(p)` keeps the player on the walkable ground. With
  `World::clamp_blocked` after it, it's passed into `Player::update` as a closure and
  also used for knockback.
- `World::clamp_blocked(p)` stops the player `BLOCK_GAP` (44 px) south of every
  undefeated road blocker. It only acts in a 60 px band just past that line, so it
  can't yank back a player who is already north of one (a dev teleport).

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
| An encounter (`Encounter`) | `world::encounter_for_chunk(k)` | Odd chunks with `k ≥ 1`, in the middle of the road. A Tide Titan always guards the first odd chunk at or past each 500 m mark, as a road blocker. Every sixth chunk from chunk 3 (`k % 6 == 3`, about every 80 m, the first at ~48 m) always has a monster, and it blocks the road. Otherwise there's a 70% chance of a fern patch. The monster comes from `MonsterKind::pick(roll, meters)`. |

Because flasks take even chunks and monsters take odd ones, they never crowd each
other.

**Spec monsters in zones** (FR21). `encounter_for_chunk` still decides *where* a zone is, whether it blocks
and which built-in (`kind`) it would hold, and none of that depends on the content dir. On top of
it, each `EncounterZone` may hold a spec monster (`spec`), chosen by the pure
`pick_spec(k, encounter, pool)`: never for a boss; otherwise `SPEC_SHARE` (40%) of zones
qualify, and one is picked by hashing the chunk over the pool's specs whose tier table
`min_m` the zone's distance has reached. With an empty pool, or none eligible, the built-in
stays, exactly as before FR21. `EncounterZone::species()` is who stands there. The **pool is the
roster frozen for the run** (the owner's decision, FR21): `World` holds it, `Game` hands it over
once the watcher's first scan has landed (`World::set_pool` rebinds the zones already
generated) and again at every `reset()` (`World::with_pool`). Specs that appear later, like the
studio's, join on the next run, so a run is a pure function of the chunk index and the content
dir as it was at the start. A reloaded spec still shows at once (`World::respec` swaps it into
the pool and the zones that hold it).

**Goalposts** (FR31). `goalposts_in_chunk(k)` lists what the tracker can point at in chunk `k`,
from the same pure functions that place the things: a milestone stone every
`GOAL_MILESTONE_EVERY_M` (100 m; the road has a stone every 50 m, too often for a goal), the
flask, and the encounter as a `Patch`, a `Blocker` or the `Titan`. Each has `y`, the
world y at which the player has *arrived* (a blocker's is its `BLOCK_GAP` line, so the
count reaches 0 m where the road stops them), and `at`, where the thing stands.
`World::next_goalpost(y)` scans the next `GOAL_SCAN_CHUNKS` chunks north, drops flasks that
are no longer `Fresh` and zones that are `cleared`, and `pick_goalpost` chooses: the nearest
north of the player, unless a higher-ranked kind (Titan, blocker, milestone, patch, flask) is
within `GOAL_RANK_SLACK_M` (25 m) of being as near. A new placed thing adds a `GoalKind`.

**Road blockers** (FR2). An `EncounterZone` with `blocks` set is a monster standing on
the road instead of hiding in ferns. It doesn't ambush: `encounter_at` skips it. The
player is stopped short of it (`clamp_blocked`), and within `BLOCKER_REACH` (70 px)
along the road `blocker_in_reach` makes it the E target. Its battle is the same as a
fern patch's (§8), and winning marks it `cleared`, which opens the road. While the
player stands at one, the curse keeps rising (§9), which is the point. The same seed gives the same world on every run and every restart.

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
   the green rune ward across the road in front of each undefeated blocker
   (`draw_ward`), and the rune stones under the flasks.
4. `world.draw_lights`: soft light pools (`gfx::glow`) on the ground under lampposts,
   fence lanterns, lit pumpkins, the campfire, cottage windows, cauldrons and the
   waiting flasks. Props say what light they cast in `Decoration::lights`.
5. **The depth-sorted pass.** `world.tall_items(&view)` returns every `Tall` item on
   screen: `Decor` (pines, dead trees, gravestones, cottages, …), fence `Post`s with
   their rails (every third pair with a lantern), `Flask`s, and undefeated road
   `Blocker`s (the monster idling, drawn through its zone's own `MonsterCanvas`;
   a boss at 1.3× the size). The player and their
   arrows (keyed by the ground under them) are added, everything is sorted by
   `sort_y()`, and the items are drawn in order. The player walks behind a tree north
   of them and in front of one south of them. The sort is stable and arrows go in
   after the player, so a fresh arrow draws over the archer.
6. `world.draw_mist`: faint drifting mist banks.
7. The curse's wisps rising off the hero (`curse::draw_wisps`, more as it worsens).
   Then `gfx::vignette` around the player's feet: clear within 190 px, fading to half of
   `NIGHT` by 620 px (`NIGHT_CLEAR_R`, `NIGHT_DARK_R`, `NIGHT_STRENGTH` in
   `game/mod.rs`). It darkens the edges without hiding anything near the player.
   Past the fever stage the curse closes it in (clear radius down to 60%, strength up
   to 0.7). Then `curse::draw_tint`, a sickly green ring closing in from the screen
   edges with the curse level, which pulses at full unless reduce flashing is on.
8. `world.draw_overhead`: bats crossing the screen and fireflies over the ground,
   on top of the darkness.
9. Key prompts (E) over the flask, road blocker or NPC in reach, drawn last so
   they're never hidden.
10. The item-get burst and banner, then the hearts and, once the curse has started,
   the curse meter under them (`curse::draw_meter`, with a tick where the fever starts).
   Both are always shown whatever the HUD setting. Then the skill bar (`skills::draw_bar`)
   at the bottom centre, hidden while the dialogue box (also bottom centre) is open and
   after death. If a battle is starting, the iris transition is drawn on top.

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
the manor's lightning becomes a steady pale glow (in both views), the full curse's
tint stops pulsing, the fireflies stop
pulsing, and the battle sky's stars stop twinkling. Any new flash, blink or shake must
check it.

---

## 8. Battles and monsters (`game/battle.rs`, `game/monster.rs`)

Walking into an uncleared fern patch, or pressing E at a road blocker
(`challenge_blocker`), calls `Game::start_battle(species, zone)`, which clears
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
goes up, and a toast shows ("The road is open." after a road blocker). There's no fleeing. Dev battles (`JEKO_DEV=jelly@…`,
`battle:<id>`) use zone index `usize::MAX`, so nothing gets cleared.

A battle is against a `Species`: `Builtin(MonsterKind)` or `Spec(Arc<MonsterSpec>)`.
`Battle::new` copies the species' `MonsterInfo` into `Battle.info`, and every rule in
the fight (hits, damage, stun, growth, hat theft, bias, XP) reads that copy. The
nameplate shows `Species::name()`, and a spec's blurb is shown under it.

`MonsterKind::info()` holds the built-ins' gameplay numbers. `MonsterKind::pick` weights the
roster by distance: Pinchlings thin out, and the tough ones grow common further up.

| Monster | Hits | Damage (½♥) | Special | From | Bias | XP/hit |
|---|---:|---:|---|---:|---:|---:|
| Pinchling | 1 | 1 | none | 0 m | 0 | 15 |
| Jellyglow | 1 | 1 | stun 2.5 s | 30 m | 0 | 20 |
| Gull Bandit | 2 | 1 | steals the hat until beaten | 50 m | 0 | 20 |
| Puffspike | 2 | 1 + misses | grows with each wrong answer | 70 m | 1 | 20 |
| Coco Knight | 3 | 2 | stun 2.0 s | 140 m | 1 | 25 |
| Dune Wyrm | 3 | 2 | rare mini-boss | 400 m | 2 | 30 |
| Tide Titan | 4 | 2 | boss, only at each 500 m mark, blocking the road | 500 m | 2 | 40 |

**Spec monsters** (FR21). A `MonsterSpec` is a monster as data: a name and blurb, a body
`shape` (`blob`, `shell`, `dome`, `segmented`, `wave`, `spiky_ball`) and size, a
three-colour palette, eyes (1–3, size, stalks, `cute`/`grumpy`/`sleepy`), up to 6 parts
(`claw`, `tentacle`, `spike`, `fin`, `wing`, `horn`, `leg`, `tail`, each with a count,
size and side), an accessory, an `idle` (`bob`, `scuttle`, `float`, `sway`, `pulse`)
and an `attack` (`lunge`, `zap`, `spin`, `slam`, `spit`). `monster_rig::draw_spec` draws
it inside `monster::draw`, so it gets the same canvas, poses, hurt flash, faint fade
and reduce flashing as the built-ins. A `theme` (`{subject, topic}`) steers its questions
(§5), and its `fact` goes on its field guide card.

A spec can't set its numbers. It picks a `tier` and a `special`, and
`MonsterSpec::info` looks them up in a table that follows the built-ins' bands. Specs
are never bosses.

| Tier | Hits | Damage (½♥) | From | Bias | XP/hit |
|---:|---:|---:|---:|---:|---:|
| 1 | 1 | 1 | 0 m | 0 | 15 |
| 2 | 2 | 1 | 50 m | 0 | 20 |
| 3 | 3 | 2 | 140 m | 1 | 25 |
| 4 | 3 | 2 | 400 m | 2 | 30 |

`special` is one of the existing mechanics: `none`, `stun` (2.0 s), `grows` or
`steals_hat`.

`content::monster_spec::parse` is the only way a spec gets in. Unknown enum values,
a bad id (1–32 of `a-z 0-9 _ -`), an empty name or a colour that isn't `#rrggbb` are
rejected. Numbers are clamped into range, text is cut (name 16 chars, blurb 80, at a
word boundary), extra fields are ignored, and a colour too dark to read against `INK`
is lightened to a contrast of at least 3.5.

**Hot reload.** The content dir is `JEKO_CONTENT_DIR`, else `./content`, else the
`content/` folder of the checkout the binary was built from. The repo ships four
hand-written specs in `content/monsters/`: Mossmunch, Lantern Moth, Bramblehog and
Inkwisp, one for each special. The watcher thread rescans `monsters/*.json` every
0.5 s and sends a `ContentEvent` for each new, edited, rejected or deleted file. A
file's `id` must match its file name. `Game::poll_content` applies the events to
`roster`:

- A rejected file is logged and shown as a toast, and the roster keeps the last good
  version.
- A reload shows a "Reloaded …" toast.
- **Live-swap rule.** If the spec being fought is reloaded, `Battle::respec` swaps in
  the new look at once, but `Battle.info` stays, so a tier or special change applies
  from that monster's next battle.

Spec monsters stand in zones too (§6, *Spec monsters in zones*): `challenge_blocker` and
stepping into ferns start the battle with `zone.species()`. The first time the player meets a
studio-made species (`gen-…`), the nameplate carries a "NEW SPECIES!" tag until the first answer
(`Battle.new_species`, from `Game.seen_generated`). `battle:<id>` still fights any spec directly.

**The field guide** (FR22, `game/field_guide.rs`, `ui/field_guide_panel.rs`). Winning a battle
calls `FieldGuide::unlock(species)`; the first time, the victory toast adds "New field guide
card!". Cards are the seven built-ins (`MonsterKind::card()`: a blurb and a true fact each) and
every spec in the roster (its blurb and `fact`), and a card that isn't unlocked shows as
"???". G or the settings modal opens the panel, and the game is paused while it's open. The
collection lasts for the session, survives `reset()` and isn't saved to disk.

**The studio** (FR21, `content/studio.rs`). With a key (or `JEKO_DEV=fakemodel`) and
unless `JEKO_STUDIO=off`, `main` starts one long-lived thread that designs monsters.
`Game::tick_studio` runs on the road frame:

- **Lowest lane.** It sets the studio's `live_busy` flag while the game is paused, a
  question is loading (the shown one or a battle's next) or a look-ahead is in flight
  (§5). The worker starts no request while it's set. It keeps at most one request in
  flight, stays under `JEKO_STUDIO_RPM` (6 a minute) and stops for the session once
  `JEKO_STUDIO_TOKENS` (60,000, about 60 monsters) are spent on monsters and lessons.
- **Stock.** It looks at the tier the player is heading into
  (`monster_spec::tier_for_meters(distance + 100 m)`) and, if fewer than 3 generated
  monsters of that tier haven't been fought yet, queues a `Job::Monster` for it, one job
  at a time, waiting about 1.5 s after a finished job for the watcher to load its file
  before counting the stock again. The job carries a maths topic that fits the tier and
  the names already taken. The dev event `genmonster` queues one for tier 2.
- **What the model returns is data, and the game keeps the rules.** `finish_monster`
  forces the requested tier, limits `special` to `none` or `grows` (`stun` and
  `steals_hat` stay with hand-written specs, provisionally), fills in the theme, makes
  the id itself (`gen-<name>`, unique) and runs `monster_spec::parse` before anything is
  written. The name, blurb, fact and theme topic are vetted against a whole-word
  blocklist (other people's characters, and anything not for a children's game). The
  prompt asks for an original, kind creature. The file is written beside its final name
  and renamed, so the watcher never sees half a spec.
- **When they reach the road.** Not until the next run: the world's pool is the roster
  frozen at the start of the run (§6), so what the studio writes now is what the *next*
  run's zones can hold. The stock rule counts unseen monsters, so it keeps a few ready for that.
- **Persistence.** Generated specs are plain files in `content/monsters/`, so they
  survive between runs and are loaded at startup like any other; the checkout's
  `.gitignore` leaves `gen-*.json` out. Renaming one (dropping the prefix, fixing its
  `id`) keeps it for committing.
- The token panel shows "studio: 3 made · 1 queued", and the `STATE` line the counts.
  Every studio call is tagged `RequestKind::Monster`.

---

## 9. Player, health and effects

- **Movement** (`Player::update`): WASD or the arrow keys at `base_speed` 140 px/s
  times `progression.speed_multiplier()` and the curse's `speed_factor()`, times
  `SPRINT_MULTIPLIER` (1.6) while Shift is held, the player is moving and
  `Stamina::can_sprint`, then the `constrain` closure. The walk
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
- **The curse** (`game/curse.rs`, FR1). The doctor's curse, felt as a fever. It's
  dormant until the first frame the player is north of the gate
  (`!world::in_hub(y)`), when it starts at 0 with a toast. From then, while `Game`
  ticks it (see §4), on the road it rises `1/FULL_AFTER_S` (45 s) per second, ramped
  up to ×1.5 (`RAMP_MAX`) by 2,000 m, and each metre of new ground (north of the
  furthest point this run) takes off `RELIEF_PER_M` (0.006), so walking at base speed
  holds it level or better. In the hub it lifts by `HUB_LIFT_PER_S` (0.05/s). A correct
  flask cleanses `FLASK_CLEANSE` (0.25). Stages: *Cursed* from 0.2, *Fever* from
  `SLOW_FROM` (0.5), where speed eases down to `SLOWEST` (×0.85) at full, and
  *Burning* at 1.0, where it drains half a heart every `DRAIN_EVERY_S` (3 s) through
  `damage_player`. Each stage toasts once on the way up, and the toasts can play again
  once it has lifted back to clear.
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
- FR22's learner data, in memory for the run only: accuracy per topic (`topics`), which
  topic is weakest, and the last four problems missed. Nothing is saved: persisting it is
  the first learner data the game would keep, which needs the owner's privacy decision.

**The UI** (`ui/`) is immediate-mode egui, rebuilt every frame from `Game`. A panel
owns no state: it reads what it's given, and the only things it changes are the
values passed to it as `&mut`.

| Panel | Anchor | Reads | Writes / returns |
|---|---|---|---|
| `hud::draw` | top right | `Progression`, `distance_m` | none |
| `hud::draw_toast` | top centre (below the badge while it's up) | `game.toast`, a top offset | none |
| `goalpost_panel::draw` | top centre | `game.goalpost.view()` (`TrackerView`) | none |
| `creation_bar::draw` | top centre, under the badge | `game.creation_view()` (`BarView`), reduce flashing | none; returns its height |
| `task_panel::draw` | left centre (bottom centre in a battle); the explanation card below (above) it | `TriggerState`, `Game.explain` | `input_buffer`, `focus_answer`; returns `Submit` |
| `token_panel::draw` | bottom right | `CerebrasClient::token_stats()`, model, base URL | none |
| `dialogue_panel::draw` | | `Dialogue` | none |
| `settings_panel::draw` | modal | | `Settings`, `settings_open`, `guide_open` |
| `field_guide_panel::draw` | modal | the guide's cards | `guide_open` |
| `map_panel::draw` | modal | `MapView` from `Game::map_view` (`Discovery` + the world) | none; M or Esc is handled in `Game::update` |
| `game_over_panel::draw` | modal | `Game` (run stats, best distance) | returns `true` to restart |

**The creation bar** (FR33). `Game.activity` (`llm::activity::GenActivity`) is advanced in
`Game::update` and opened and closed where questions are asked and land: `request_task` opens a
`Question` job (labelled `flask` or `battle`) for a live call, `poll_questions` ends it `Ready`
when the answer lands or `Fallback` when the call failed or missed the soft deadline, and
`stash_questions` cancels the ones let go of (their answers still go to the buffer). A question
taken from the buffer makes no job, since nothing was generated; with no API key the bank's
answer shows as an instant `Fallback` job. The ring is a stage ring, not a percentage: an arc
turns while the job is `Writing`, closes when it ends, the icon pops with a spark (no spark and
a slower arc with reduce flashing), and the time it took shows underneath (`bank` for a
fallback). A job still open after 12 s is dropped, and an ended one stays 2.2 s. Three circles at
most, then "+N". Prefetch, monster and lesson kinds are in `RequestKind` but nothing opens
them yet. It never changes XP, HP, answers or timing (invariant 7).

**The goalpost badge** (FR31). `Game::tick_goalpost` asks `World::next_goalpost` for the next
thing up the road (§6) and feeds it to `goalpost::Tracker`, which owns the animation: a
circle pops in holding the goalpost's icon, unrolls into a pill (`name · N m`, metres rounded
up so it reads 0 on arrival), a ring around the circle fills as the player closes in and
pulses under 10 m, "Now!" shows for 0.8 s on arrival, then it rolls up, shrinks away and the
next one pops in. It steps aside (rolls up and stays gone) in a battle, a dialogue, while a
flask's problem is loading or open, in the hub, and when `show_hud` is off. It runs on the
game clock, so a pause freezes it, and it never changes XP, speed or answers (invariant 7).

`ui::style::apply` sets the theme every frame: pale parchment panels (kept light so
text reads against the dark world), thick dark-wood borders, hard offset shadows, a
lantern-amber hover and a moss-green accent. `gfx::key_prompt` copies the palette, so world
prompts match the panels. `Settings` (`show_hud`, `show_token_panel`,
`show_creation_bar`, `reduce_flashing`) lives on `Game` and isn't saved between launches.

---

## 11. Content, configuration and dev hooks

**Assets** are compiled into the binary with `include_str!`, so editing them needs a
rebuild. The exception is the content dir, which is read at runtime and hot-reloaded
(`content/monsters/*.json`, `lessons/*.json` and `tips/*.json`, §8, §5):

| File | Loaded by | Used for |
|---|---|---|
| `assets/tasks_fallback.json` | `llm::fallback_bank` | Questions when there's no key or a call fails. Must deserialize as `Vec<MathTask>` and be non-empty. |
| `assets/dialogue.json` | `dialogue::load_bank` | NPC lines. Professor Pebble says the hub tutorial and then rotates through their bank lines. Regenerate it with `make gen-content`. `dialogue::check_bank` is the rule for it (3 to 12 lines per NPC, at most 140 characters, the studio's word blocklist, unique names, Professor Pebble present): `gen_content` runs it before writing, `make validate` runs it on the file, and a test runs it on the shipped file, because `load_bank` panics at startup on a bad one. |

**OpenClaw (FR30, optional).** `openclaw/` holds skills for the OpenClaw agent gateway, one
per job. Nothing in `src/` calls OpenClaw and the game doesn't know it exists: an agent
only runs the repo's own tools (today `make gen-content` then `make validate`, in
`openclaw/skills/jeko-dialogue/`) and leaves the result in the working tree for review.
Setup, the agent's tool limits and the safety notes are in `openclaw/README.md`.

**Configuration.** `CEREBRAS_API_KEY` is optional; without it the game runs offline.
`CEREBRAS_MODEL` defaults to `kimi-k3.1`, a placeholder slug.
`CEREBRAS_BASE_URL` defaults to `https://api.cerebras.ai/v1`. Values come from the
environment, a `.env` file, or secretspec (`secretspec.toml`). See the README.
`JEKO_CONTENT_DIR` points the content watcher at another folder (§8).
`JEKO_PREFETCH=off` turns the question look-ahead off, and `JEKO_PREFETCH_RPM` (default 20)
caps its requests per minute (§5). `JEKO_STUDIO=off` turns the monster studio off, and
`JEKO_STUDIO_RPM` (6) and `JEKO_STUDIO_TOKENS` (60,000) bound it (§8).
`JEKO_INSTANCE` names the checkout for the window title (`JEKO — Math Quest [name]`)
and the startup stderr line. It defaults to the build folder's name, so two clones
run side by side can be told apart. Nothing else is shared between clones: each has
its own `target/`, `.env` and `content/`, and the game opens no ports and writes no
files apart from a `JEKO_SHOT` screenshot.

**Dev hooks** (`main.rs` plus `Game::dev_event`) let a change be checked without
driving the window:

- `JEKO_SHOT=out.png:120` saves a screenshot at frame 120, prints
  `Game::debug_summary()` (a `STATE …` line) to stderr, and exits.
- `JEKO_DEV=name@frame,…` fires scripted events: damage (`hurt`, `kill`), moves
  (`jump`, `roll`, `shoot`, `stun`, `cheer`), `spent` (empty the roll charges and
  stamina, to show the skill bar recovering), `restart`, `pause`, `settings`, `map` (open the map), `far` (teleport 120 m up the road and discover it), battles
  (`pinch`, `jelly`, `puff`, `coco`, `gull`, `wyrm`, `titan`, and `battle:<id>` for a
  spec monster from the content dir), answers through the
  real submit path (`right`, `wrong`), `e` (press E), teleports (`toflask`,
  `tozone` to the first fern patch, `toblocker` to the line in front of the first
  road blocker, `topebble`, `toroad` 40 m up the road, `neargoal` 8 m short of the next
  goalpost, `atgoal` onto it), the curse (`curse` at 0.75, the fever stage; `cursefull`
  at 1.0), and display settings: `nohud` (hide both side panels, so
  a shot shows the whole world) and `calm` (turn on reduce flashing).
- `JEKO_DEV=fakemodel:700` (no `@frame`) swaps the model for a stand-in that answers every
  call from the fallback bank after 700 ms, with made-up token counts, so the buffer, the
  soft deadline and the token panel's wait readout can be tried with no key. The studio
  uses it too: it writes made-up creatures, so the whole path from job to a fightable
  monster runs offline. `genmonster@N` queues one tier 2 job, `gen:question` (also `gen:fallback`, `gen:monster`, `gen:lesson`, `gen:slow`) queues a fake creation-bar job that writes for 2.5 s (10 s for `slow`) and ends, `cards` unlocks four field guide cards and
  `guide` opens the guide. `tospec` and `tospecblock` teleport to the first
  zone (or road blocker) that holds a spec monster.
- With either variable set, the loop steps a fixed 1/60 s, so a run is repeatable
  frame by frame. Without an API key, questions come straight from the fallback
  bank, so `right`/`wrong` work on the next frame.

**Tests** are unit tests inside the modules. Run them with `make test`, which runs
`cargo test` in the nix dev shell.

| Module | What's covered |
|---|---|
| `content/monster_spec.rs` (8) | A good spec loads with defaults. Huge and negative numbers are clamped. Unknown enum values are rejected. Bad ids, empty names and malformed colours are rejected. Long or multi-line text is cut and cleaned. Too-dark colours are lightened until they read against `INK`. Stats come from the tier table only, whatever extra fields say. The tier for a distance matches the table's `min_m`. |
| `content/watcher.rs` (4) | A missing dir is empty. A file loads, an edit reloads it, a broken edit and a mismatched id are reported, and a delete is reported. Lesson packs and tips load and reload, a wrong answer key is dropped on load and an unusable pack is reported. The specs shipped in `content/monsters/` all load. |
| `content/lessons.rs` (9) | A good reply becomes a pack file that loads back, never overwriting; a wrong answer key, an unparseable expression, an empty step, off-limits words and overlong text are dropped and never written; bad replies write nothing; the pack is capped; a hand-edited wrong answer is caught on load; topics normalise. |
| `content/tips.rs` (3) | A good reply becomes a tips file that loads back; bad lines are dropped and an empty result is an error; the prompt names the topic. |
| `game/field_guide.rs` (4) | Every built-in has a card with a fact; a card unlocks once and stays; spec monsters get cards after the built-ins; ids never collide. |
| `progression/` (3) | Answers count per topic ignoring case; the weakest topic needs a few tries and a poor score; missed problems come back once, oldest first, and only a few are kept. |
| `content/studio.rs` (12) | A good reply becomes a loadable file with the tier forced and the special limited. Ids are unique, fit the limit and alphabet. Bad, off-limits and hostile replies are rejected or clamped and write nothing. Whole-word vetting. The stand-in model's creatures always pass. The worker writes one file per job, holds jobs while a live question is loading, obeys the minute cap and stops at the token budget. No key or switched off means no studio. |
| `content/prompts.rs` (5) | The shipped example is valid, every value the prompt lists is accepted by the parser, the prompt carries the tier, topic and names to avoid, and every level has topics that fit a theme. |
| `content/mod.rs` (1) | A rejected file keeps the last good version in the roster; a delete removes it. |
| `game/monster_rig.rs` (2) | The palette check's `INK_RGB` matches `gfx::INK`; parts spread evenly. |
| `game/world.rs` (30) | Scenery never lands on the road. Every flask is reachable and on the road. Generation is deterministic. Interact reaches only nearby fresh flasks. The clamp keeps the player on the road. Encounters are on the road and deterministic. A Titan guards every 500 m, blocking the road. Monsters block the road regularly (no gap over 100 m, the first under 70 m) and deterministically. A blocker shuts the road, can be challenged but doesn't ambush, and opens once beaten. Tougher monsters appear further up. The hub is wide, dry and bounded. The walkable width is continuous through the gate. Goalposts are deterministic and stand in their chunk, nothing hostile is one in the hub, the pick is always north of the player, nearest first and the bigger event first within 25 m, a solved flask or beaten monster is never offered again, and a blocker counts down to its line. `upcoming_in` is nearest first and in range, matches where the world places things, shows no monster in the hub, takes a monster's hits and bias from its stats, and `World::upcoming` leaves out what's been dealt with. About 40% of zones get a spec monster and never a boss, one far enough up the road; an empty pool changes nothing; the same pool gives the same zones and the layout never depends on it; rebinding a built world matches building it with the pool; an edited spec shows in its zones; `upcoming` uses a held spec's tier. |
| `game/goalpost.rs` (7) | A new goalpost pops in as a circle and then unrolls. The metres go down as the player walks, rounded up, and the ring fills. It pulses under 10 m and says "Now!" on arrival. It closes before the next one appears, hides while a battle or problem is open and comes back, and shows nothing with nothing ahead. Labels. |
| `game/manor.rs` (1) | Lightning strikes now and then (under 12% of frames), stays in 0..1, and is the same every run. |
| `llm/activity.rs` (10) | A job writes, then is ready and remembers how long it took; a failure shows as fallback; ending twice keeps the first ending; a finished job leaves after lingering; one nobody ended goes stale; cancel; the bar shows three circles and counts the rest; the pop and fade; an instant job; a scripted job ends by itself. |
| `llm/usage.rs` (6) | Provider-timed vs round-trip speed, zero time ignored, no usage block, the capped history, totals split by kind. |
| `llm/expr.rs` (5) | The evaluator follows the usual rules (precedence, parentheses, unary minus, `×` `÷` `−`), rejects anything that isn't plain arithmetic (powers, functions, division by zero, `1 2` as 12), bounds hostile input (nesting, length), and `verify` passes a problem only when its answer matches its expression. |
| `llm/mod.rs` (2) | The compiled-in bank loads, every answer in it checks out and has an explanation; a task without the FR22 fields still loads. |
| `llm/task_gen.rs` (3) | The live prompt asks for an expression and explanation and takes a topic; the pack prompt names count, topic, level and what was missed; JSON is pulled out of chatty replies. |
| `llm/buffer.rs` (11) | The buffer gives the exact difficulty, else one below, never a harder one, and prefers a topic. `plan_prefetch` asks for what's missing nearest first, counts ready and in-flight questions, and obeys the in-flight, per-difficulty and per-minute caps. `WaitStats` reads out as the panel shows it. A topic takes only that topic; a question isn't added twice. |
| `game/prefetch.rs` (7) | A flask wants one question and a monster its hits plus a spare, clamped to difficulty 5. Env switches parse. Landed answers reach the buffer and failures vanish. A disabled prefetcher asks for nothing. A missed problem comes back first as similar practice; a thin buffer asks for the weakest topic, else the rotation; a stocked one asks for nothing. |
| `game/bow.rs` (2) | The shot draws, then releases, then lowers. An arrow flies the way it was loosed, comes down a believable distance away, then goes away. |
| `game/health.rs` (4) | Starts full; damage, heal and refill clamping. |
| `game/curse.rs` (8) | Nothing happens in the hub before the road. It starts the first time the player is past the gate. Standing still fills it, then drains on the interval. Walking north holds it back, even at the far end of the ramp. Old ground gives no relief. The hub lifts it. Stage toasts replay only after it clears. Cleanse and speed clamp. |
| `game/skills.rs` (5) | Three rolls then a lockout that refills all; a spent charge trickles back; sprinting drains and running dry winds; stamina waits, then recovers, and winded ends at the threshold; a short break doesn't refill. |
| `game/map.rs` (6) | Discovery starts at the gate and only moves north. The range has a floor and grows with progress. Landmarks never sit in the fog, while flasks further up exist but stay hidden. Milestones every 50 m. The same walk gives the same map. Solved flasks show as solved. |
| `game/dialogue.rs` (4) | The shipped bank passes `check_bank`; `check_bank` rejects hostile banks (not a list, no Pebble, too few or too many lines, an empty or over-long line, a blocked word, a missing name, a duplicate); the typewriter reveals, then advances, then closes. |
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
   `.await` or block on a request inside `update`/`draw`. Reading the content dir
   follows the same pattern (the watcher thread).
2. **Every question path works offline.** No key, a timeout, an HTTP error and a bad
   payload all fall back to `assets/tasks_fallback.json`. A new question source needs
   the same fallback.
3. **The world is a pure function of `y` and the chunk index** (and, for which spec monster
   stands in a zone, of the run's frozen roster: the owner's decision for FR21, so a
   run is reproducible given the content dir). Generation uses only
   seeded `StdRng` or `gfx::hash01`, never `thread_rng`, so it's the same every run and
   the placement tests stay meaningful. Where things are never depends on the content dir. New placed things (like the road blockers,
   and the planned cages and shells) should follow `spot_for_chunk` / `encounter_for_chunk` and get a
   determinism test.
4. **The hub is a safe zone.** Nothing hostile spawns south of `ROAD_SOUTH_END`
   (chunk `k < 1`), the curse doesn't start until the player first leaves it, and it
   lifts there rather than growing.
5. **`Game` owns run state, and panels are views.** UI functions take what they need
   and return events. They don't keep gameplay state of their own.
6. **One door per rule.** HP changes go through `damage_player`, `Health::heal` and
   `Health::refill`. Questions start through `request_task`. Answers are judged in
   `submit_answer`.
7. **Cosmetic means cosmetic.** `Action`s, the bow's arrows, the planned pet cat (FR33)
   and visual effects never change XP, speed, answers or battle results.
8. **Animation runs on the game clock.** Use `Game.time` or per-object timers advanced
   by `dt`, so pausing freezes everything.
9. **Reduce flashing is honoured** by every flicker, flash and shake.
10. **Layering** (§2): `config`/`content`/`llm`/`progression` stay free of macroquad and egui,
    `game` stays free of egui.

---

## 13. Known limits and open design

These are known gaps, not bugs. Each one is either filed as an FR or a decision
still to make:

- **No persistence.** Progress, settings and best distance live only in memory (FR14
  and FR16 record this as provisional).
- **Answers are numeric only.** `MathTask.answer` is an `f64` with an exact-match
  check. Non-math subjects or multiple choice (an FR3 question, open in FR16) would need a
  new answer type through the prompt, the parser, `task_panel` and `submit_answer`.
- **One thread and one HTTP client per request**, with no cancellation or retry. Look-ahead
  now overlaps requests (at most two prefetches plus a live one), which is still fine. The
  fix, one long-lived worker with a priority queue, is half there: the studio is a long-lived
  thread, but live and look-ahead questions still spawn a thread each, and the studio's
  lane is kept by a flag (`live_busy`), not by a shared queue.
- **Look-ahead can waste calls.** It asks for questions the player may never reach (they die
  first, or skip a flask), so it costs up to about 1.5 times the calls. `JEKO_PREFETCH_RPM`
  bounds it.
- **The difficulty cap is 5** and comes from level plus monster bias only. It doesn't
  rise with distance.
- **The hero and the monsters still dress for the beach.** FR24 changed the world
  but not the characters: the hero wears a straw sun hat and swim shorts, and the
  roster is crabs, jellyfish, pufferfish, coconuts, gulls, a sand worm and a Tide
  Titan. Restyling them is FR25.
- **Learner data isn't kept.** Per-topic accuracy, the missed problems and the field guide live in
  memory and are gone on exit (FR14); whether to persist them is a privacy decision for the owner.
- **Questions are numeric and maths-only.** Lesson packs, themes and the field guide's facts
  stay inside that (FR3's open question, FR16).
- **The studio's monsters wait for the next run.** The zone pool is the roster frozen at
  the start of a run, so a monster written mid-run first appears on the road after a
  restart (§6). On a fresh checkout the first run has only the hand-written specs.
- **The studio's prompts haven't been checked against the real model.** The monster, lesson
  and tips paths run end to end with the stand-in model and are covered by tests, but the sandbox
  they were built in had no network, so a real reply's quality (and how often a reply is turned
  down) is unmeasured. The live question prompt now also asks for an expression, and a reply
  without a checkable one falls back to the bank, so a model that ignores it would make every
  question a bank question. `make gen-monsters N=8` is the way to look at monsters.
- **The settled-for-now defaults** from FR1–FR3, FR5–FR15, FR20, FR24, FR26 and FR31 are listed for the owner in FR16.

See [FeatureRequestToDo.md](../FeatureRequestToDo.md) for the backlog.
