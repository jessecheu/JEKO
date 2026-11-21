---
type: Feature Request
title: JEKO — Feature Requests
description: Concrete, actionable feature requests for JEKO (Math Quest) — work that is scoped and wanted but not yet built. Each entry states the motivation, a proposed approach, and what "done" looks like. Maintained as the shared agent todo list.
tags: [feature-request, backlog, gameplay, urgency, lava, monsters, battle, education, minigames, pets, controls, pause, settings, health, death, art-direction, spawn]
timestamp: 2026-11-21
---

# JEKO — Feature Requests

Scoped, wanted-but-unbuilt work. Each entry frames the motivation, a proposed
approach, and acceptance criteria, so it can be picked up without re-deriving the
context. When a request is built, fold it out (see below).

## How to use this as a todo list

This file is the shared todo list for feature work — the handoff point between
agents. Treat it as living state, not a static backlog:

- **Before starting**, read this file to pick up where the last agent left off.
  Check **Depends on** first: it's an array of the FR numbers that must land
  before this one (`[]` = no blockers, ready to start). The italic note beside it
  records softer couplings — what an FR pairs with or unblocks but isn't gated by.
- **As you work**, keep the entry current: tick its `- [ ]` acceptance criteria as
  they pass, update its **Status** line, and add newly discovered follow-ups to it.
- **If you stop mid-feature** (a cliffhanger), record exactly what is done, what
  remains, and the next concrete step in the entry's **Status** line, so the next
  agent can resume without re-deriving the context.
- **Open questions are the owner's to answer.** Don't settle one silently in code;
  ask, then record the answer in the entry.
- **When a request is fully built, fold it out — this is a step, not a tidy-up.**
  Delete its entry here, add its one-line row under [Completed](#completed), and
  note *why it closed* there. An entry stays only while it has unbuilt work of its own.

Entries are grouped by area. Numbering is stable; don't renumber when one is removed.

## Numbering

**The next free number is FR19**; bump it when you file one. Numbers are never
reused or renumbered when an entry is removed. Take the next number, not the first gap.

---

# Gameplay — urgency

**Goal:** make the player feel urgency so they keep moving up the boardwalk.
Today nothing pushes the player: they can stand still, and the only stops are the
message-in-a-bottle problems (`src/game/mod.rs` freezes the player while a problem
is `Loading` or `Active`). FR1 pushes from behind; FR2 and FR3 block the way ahead.

## FR1. Rising lava from the bottom of the road

**Status.** **Not started.**

**Depends on:** [] _(pairs with FR2 — the lava is the pressure that makes a blocking monster matter)_

**Motivation.** The player can stand still forever. A front that keeps advancing up
the boardwalk behind them means they can't, and gives every stop (a bottle, a
monster) a cost.

**Proposed approach.** *(suggested, not decided)* Keep a world-space lava front
`lava_y` on `Game`, starting a little south of `world::PLAYER_START_Y` and moving
north (towards smaller `y`, since `distance_m` grows as `y` falls) every `update`.
Draw everything south of it as lava over the deck and sand in `World::draw_ground` /
`draw_boardwalk`. Ramp its speed with distance or `progression.level`. Decide what
the lava does while the player is frozen at a problem (see open questions): if it
keeps full speed, slow model responses punish the player, which works against the
README's "the problem should appear fast enough that it doesn't break movement flow".

**Open questions.**

- What happens when the lava reaches the player — game over and restart, knock back
  to the last solved bottle, or lose the streak?
- Does the lava pause, slow, or keep going while a problem or battle is open?

**Acceptance criteria.**

- [ ] The boardwalk behind the player fills with lava that keeps advancing, so the
  player can't stand still and has to keep going.
- [ ] The lava reaching the player has a defined outcome (per the open question above).
- [ ] The lava's behaviour during an open problem/battle is defined and implemented.

---

## FR2. Monsters blocking the road ahead

**Status.** **Not started.**

**Depends on:** [] _(FR3 makes the battle itself; until FR3 lands, a placeholder interaction can clear a monster)_

**Motivation.** The lava (FR1) pushes from behind; a monster on the road ahead is
something the player has to deal with *now*, with the lava coming.

**Proposed approach.** *(suggested, not decided)* Spawn monsters per chunk the way
bottles are (`spot_for_chunk` in `src/game/world.rs`), so placement stays
deterministic and testable like the existing `every_bottle_is_reachable_and_on_the_deck`
test. An undefeated monster blocks the road: clamp the player's `y` so they can't
pass it (alongside `world::clamp_to_path`). Walking up to it and pressing E starts
the battle, through the same interact flow as `interactable_spot`. Draw it with
flat primitives like the rest of the world (`gfx.rs`, `decor.rs`).

**Open questions.**

- Do monsters replace bottles, or do both appear on the road?
- How often do monsters appear, and does that scale with distance or level?

**Acceptance criteria.**

- [ ] Monsters show up on the road in front of the player.
- [ ] The player can't get past a monster without battling it.
- [ ] Monster placement is deterministic and covered by a test, like the bottles'.

---

## FR3. A battle is an education Q&A

**Status.** **Partly built via FR10.** Wild-encounter battles (`src/game/battle.rs`) already ask questions through the bottle pipeline, with the fallback bank, and a monster is beaten by enough correct answers. Still open: tie this to FR2's road-blocking monsters, and the open questions (non-math subjects, cost of a wrong answer; FR10 provisionally uses monster damage).

**Depends on:** [FR2]

**Motivation.** The game's premise is learning through play: the battle should be the
same question-answering the bottles do, with higher stakes because the road is shut
until the player answers correctly.

**Proposed approach.** *(suggested, not decided)* Reuse the bottle pipeline: the
`TriggerState` machine (`src/game/task_trigger.rs`), the Cerebras request
(`CerebrasClient::request_task_async`, prompt in `src/llm/task_gen.rs`), the fallback
bank (`assets/tasks_fallback.json`), and the answer panel (`src/ui/task_panel.rs`).
A correct answer defeats the monster, unblocks the road, and awards XP through
`progression.record_correct`. On a wrong answer the monster stays and the player
retries with a fresh question after the cooldown, while the lava keeps its pressure.

**Open questions.**

- Are the questions math only (what the prompt asks for today), or other school
  subjects too? Today `MathTask.answer` is a number, so non-math questions would
  need a different answer type, such as multiple choice.
- What does a wrong answer cost — just time, or also HP or the streak?

**Acceptance criteria.**

- [ ] Fighting a monster means answering education questions.
- [ ] Answering correctly wins the battle and clears the road.
- [ ] A battle works without an API key or when the live call fails (fallback bank),
  like the bottles do.

---

# Gameplay — minigames & companions

**Goal:** break up the walk with small optional moments that reward the player
without changing the game's balance. FR4 is the first: a caged pet lion. It sets
the pattern (a thing on the road, a quick puzzle in a modal, a cosmetic reward)
that later minigames can copy.

## FR4. Unlock a pet lion from a cage

**Status.** **Not started.**

**Depends on:** [] _(pairs with FR2/FR3 — the lion's "help" is most visible at a monster battle; until those land, it reacts to bottles)_

**Motivation.** A short puzzle on the road is a change of pace from the
message-in-a-bottle problems, and a companion that follows you up the boardwalk
gives the player something to show for it. The lion only *looks* like it helps:
its "help" is animation. It never changes XP, speed, answers or battle outcomes.

**Proposed approach.** *(suggested, not decided)*

- **The cage.** Place a locked cage with a lion inside on the road, deterministic
  per chunk like the bottles (`spot_for_chunk` in `src/game/world.rs`), so it can be
  tested the same way as `every_bottle_is_reachable_and_on_the_deck`. Draw the cage
  and the lion with flat primitives (`gfx.rs`, `decor.rs`) and depth-sort them with
  the player through `Tall` / `tall_items`.
- **The puzzle modal.** Walking up to the cage and pressing E (`INTERACT_KEY`) opens
  an egui modal, built like `src/ui/task_panel.rs` (e.g. a new
  `src/ui/puzzle_panel.rs`). The puzzle is quick and solving it gives the player a
  key. One idea: a combination lock whose digits come from a small arithmetic clue,
  which keeps the learning theme and needs no API call. Give the puzzle its own
  state rather than reusing `TriggerState`, so it can't collide with an open bottle
  task, and add it to the `busy` check in `Game::update` so the player stops while
  the modal is open.
- **Unlocking.** Using the key opens the cage with a short animation (door swings,
  lion hops out), and the lion becomes the player's pet.
- **Following and "helping".** The lion trails the player with a little lag,
  using `world::clamp_to_path` so it stays on the deck, with idle and walk
  animations (tail swish, pad cycle). Its "help" is animation only: it cheers or
  pounces when the player answers correctly, droops on a wrong answer, and roars
  at a monster during a battle (FR2/FR3). Keep the lion out of `Progression` so it
  can't affect gameplay.

**Open questions.**

- Is the puzzle math (a clue that sets the lock's combination), or a non-math
  logic puzzle such as sliding tiles or matching a key's shape?
- Does the cage sit on the road and block it until opened, or beside it as an
  optional detour the player can skip?
- Is there one lion per run, or can more cages (and other pets) show up further
  along? How often?
- What happens on a failed or abandoned puzzle — retry right away, a cooldown like
  the bottles, or the cage is gone?
- Does the lion survive a lava game-over/restart (FR1)? Nothing is saved between
  runs today.

**Acceptance criteria.**

- [ ] A locked cage with a lion appears on the road, placed deterministically and
  covered by a test, like the bottles.
- [ ] Pressing E at the cage opens a puzzle modal, and the player can't move while
  it's open.
- [ ] Solving the puzzle gives the player a key that opens the cage, with an
  unlock animation.
- [ ] The freed lion follows the player up the boardwalk, stays on the deck, and
  draws in the right depth order with the player and scenery.
- [ ] The lion plays "helping" animations (at least on correct and wrong answers),
  and having the lion leaves XP, speed and answer checking unchanged.
- [ ] The puzzle works without an API key.

---

# Owner decisions

**Goal:** close the open questions FR5–FR15 asked. Those features are built
(see [Completed](#completed)), each on a provisional answer so the work could
land; this entry keeps the questions so they are answered, not settled silently.

## FR16. Confirm the provisional answers from FR5–FR15

**Status.** **Waiting on the owner.** Every default below is live in the code.
Tick an item once the owner confirms it, or replace it with the owner's answer
and file the change as its own FR if it needs code.

**Depends on:** [] _(answers here may spawn small follow-up FRs)_

**Motivation.** FR5–FR15 were built in one pass. Their open questions are the
owner's to answer, so each was given a reasonable, easily changed default and
recorded here.

**Decisions to confirm** (question, then the provisional answer in the code):

- [ ] **FR5** Jump/roll while standing still? *Yes.* Roll as a dodge later? *Not now;
  the roll has a defined start/end (`Action::Roll { t }`) so one can hook in.*
- [ ] **FR6** Pause with a bottle task open? *Allowed* (the problem stays readable).
- [ ] **FR7** First-version settings? *Show progress panel, show token panel, reduce
  flashing, controls reference.* Persist between runs? *No.* Difficulty override?
  *No, `progression.level` drives it.*
- [ ] **FR8** Move during the spawn flicker? *Yes.* Invulnerable while it plays?
  *Yes* (`Player::spawn_protected`).
- [ ] **FR9** Hero: refresh or new character? *Refresh* (bolder ink, bigger head and
  eyes, pouch). Pixel font? *No, egui default with chunkier frames.* Collectibles?
  *Filed as FR17, not built.*
- [ ] **FR10** Wild encounters vs FR2 blockers? *Both can coexist; FR2 not built.*
  Run from a battle? *No.* Catch/befriend? *No.* How often? *~70% of odd road
  chunks; bottles take the even ones.* Roster? *All 7 shipped: Pinchling,
  Jellyglow, Puffspike, Coco Knight, Gull Bandit, Dune Wyrm (rare from 400 m),
  Tide Titan (boss every 500 m).*
- [ ] **FR11** Knockback? *Yes, 22 px away from the source when it has one.*
  Invulnerability window? *1.0 s* (`HURT_INVULN_S`).
- [ ] **FR12** What a stun does? *Can't move and can't submit an answer until it
  ends.* Stacks with damage? *Yes, with the normal invulnerability window.*
- [ ] **FR13** Hearts or number? *5 hearts in half-hearts.* Sources/heals? *Monster
  hits (1–2 half-hearts; Puffspike grows); correct bottle heals ½ heart; level-up
  refills.* Wrong **bottle** answer costs HP? *Yes, 1 heart.*
- [ ] **FR14** Progression on death? *Everything resets.* Same world each run? *Yes
  (hash-seeded).* Persist a best distance? *No, session only.*
- [ ] **FR15** Hub size/contents? *~1.3 screens of scenery plus Professor Pebble's
  tutorial.* Walk back into the hub? *Yes.* Restart in the hub? *Yes.*

**Acceptance criteria.**

- [ ] The owner has confirmed or replaced every decision above, and any change
  that needs code is filed as its own FR.

---

# Gameplay — collectibles

## FR17. Shells as collectible coins

**Status.** **Not started.**

**Depends on:** [] _(raised by FR9's "World touches"; FR9 itself shipped without it)_

**Motivation.** FR9 suggested bouncy collectibles (shells spinning like coins) and
a counter in the HUD. FR9 noted collectibles with a *use* would be their own FR.

**Proposed approach.** *(suggested, not decided)* Place shells per chunk
deterministically like bottles (`spot_for_chunk`), spin and bob them, collect on
touch with a small sparkle (reuse `gfx::item_get`'s burst), and count them in the
HUD. Give them a use (e.g. a shop in the FR15 hub, or extra hearts).

**Open questions.**

- What are shells for: score only, a shop, healing?
- Do they reset on death (FR14)?

**Acceptance criteria.**

- [ ] Shells appear on the road deterministically, covered by a test.
- [ ] Walking over one collects it with feedback, and the HUD shows the count.
- [ ] Shells have the agreed use.

---

## Completed

When an entry is fully built, delete it above and add one line here, newest
first: `FR<n> — title — date — why it closed (commit)`. Its number stays retired.

- FR18 — Sprint on Shift, roll moved to Ctrl — 2026-11-21 — built: holding Shift
  multiplies walk speed by `SPRINT_MULTIPLIER` (1.6, stacks with Speed Boost) in
  `Player::update`; roll rebound to Ctrl; controls reference updated (uncommitted)
- FR9 — Classic adventure-platformer look & feel — 2026-11-21 — built: refreshed
  hero (bolder ink, bigger eyes, pouch), "item get" pose with `gfx::item_get` rays,
  sparkles and "+N XP!" banner on a solved bottle, framed egui style with hard drop
  shadows (`ui/style.rs`), hearts (FR13), Professor Pebble in the hub with
  typewriter dialogue (`game/dialogue.rs`, `ui/dialogue_panel.rs`). Pixel font
  declined provisionally (FR16); collectibles split out as FR17 (uncommitted)
- FR10 — Monster roster, wild encounters and battle animations — 2026-11-21 — built
  ahead of FR2/FR3 with wild encounters: deterministic sea-grass zones
  (`world::encounter_for_chunk`, tested), iris-wipe battle (`game/battle.rs`) whose
  Q&A reuses the bottle pipeline and fallback bank, all 7 monsters with idle, intro,
  attack, hurt and faint (`game/monster.rs`), toughness scaling with distance and a
  Tide Titan boss every 500 m (tested). FR2's road blockers remain open; FR3 is
  partly covered (see its Status) (uncommitted)
- FR15 — A bigger spawn area that leads onto the road — 2026-11-21 — built: hand-placed
  hub south of the gateway arch (`decor::hub_layout`), funnel clamp `walkable_x`,
  distance and markers from `ROAD_START_Y`, tests for hub bounds and gateway
  continuity (uncommitted)
- FR12 — Stun status with a halo-of-stars animation — 2026-11-21 — built: orbiting
  stars with depth, spiral eyes, wobble, pop on end; Jellyglow and Coco Knight stun;
  stunned players can't move or answer (provisional, FR16) (uncommitted)
- FR14 — Death and restart from the start — 2026-11-21 — built: flop-and-fade death,
  Game Over panel with run stats, `Game::reset()` restart without relaunching;
  checked end to end with a scripted run (uncommitted)
- FR11 — Damage flicker when the player is hurt — 2026-11-21 — built on FR8's
  `Flicker` (`FlickerKind::Damage`) inside `Game::damage_player`: red hit frame,
  blink, knockback, shake, 1 s invulnerability (uncommitted)
- FR13 — Player HP — 2026-11-21 — built: `game/health.rs` (half-hearts, unit-tested),
  single `Game::damage_player` entry point, hearts always drawn top-left (uncommitted)
- FR8 — Ghost flicker on spawn — 2026-11-21 — built: reusable `game/flicker.rs`,
  seam-free via the player's render-target blit, honours reduce flashing (uncommitted)
- FR5 — Jump (Space) and roll (Shift, now Ctrl per FR18) animations — 2026-11-21 — built: cosmetic
  `Action` on `Player`, render-target blit for rotation and squash (uncommitted)
- FR7 — Settings modal on O (pauses the game) — 2026-11-21 — built: `Settings` on
  `Game`, `ui/settings_panel.rs`; closing keeps a P pause (uncommitted)
- FR6 — Pause the game with P — 2026-11-21 — built: `Game.paused` with a game clock so
  animation freezes; LLM responses still polled; P in the answer field ignored
  (uncommitted)
- FR4 — Token usage panel (model, API address, tokens/sec, per-request speed graph,
  token totals) — 2026-11-20 — requested directly and built in one pass, so it never
  sat open above: `src/ui/token_panel.rs` reads `llm/usage.rs` stats that
  `CerebrasClient` records from each response's `usage` / `time_info` (uncommitted)
