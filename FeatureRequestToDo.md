---
type: Feature Request
title: JEKO — Feature Requests
description: Concrete, actionable feature requests for JEKO (Math Quest) — work that is scoped and wanted but not yet built. Each entry states the motivation, a proposed approach, and what "done" looks like. Maintained as the shared agent todo list.
tags: [feature-request, backlog, gameplay, urgency, curse, sickness, monsters, battle, education, minigames, pets, cat, controls, pause, settings, health, death, art-direction, spawn, tooling, ci-cd, dagger, content-generation, hot-reload, live-content, villain, story, live-dialogue, setting, forest, manor, map, exploration, regions, village, castle, rest-stops, road, obstacles, agents, openclaw, hud, goalposts]
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

**The next free number is FR34**; bump it when you file one. Numbers are never
reused or renumbered when an entry is removed. Take the next number, not the first gap.

---

# Gameplay — urgency

**Goal:** make the player feel urgency so they keep moving up the road.
The doctor's curse (FR1, built) now wears the player down while they linger on the
road, and monsters standing on the road (FR2, built) shut it until they're beaten
in a question battle (FR3, built). Both are done; their open questions are in FR16.

# Gameplay — minigames & companions

**Goal:** break up the walk with small optional moments that reward the player
without changing the game's balance. FR33 is the first: a caged pet cat. It sets
the pattern (a thing on the road, a quick puzzle in a modal, a cosmetic reward)
that later minigames can copy.

## FR33. Unlock a pet cat from a cage

**Status.** **Not started.** Reworked from a pet lion to a pet cat at the owner's
request (2026-11-21), and renumbered from FR4, a number the token usage panel
already had (see Completed), then from FR32 to FR33 because the prefetch FR
(*Live content*) held FR32 too; the approach is suggested only.

**Depends on:** [] _(FR2 and FR3 are built, so the cat's "help" at a battle can be
built in the same pass; it reacts to flasks too)_

**Motivation.** A short puzzle on the road is a change of pace from the
potion-flask problems, and a companion that follows you up the road
gives the player something to show for it. The cat only *looks* like it helps:
its "help" is animation. It never changes XP, speed, answers or battle outcomes.

**Proposed approach.** *(suggested, not decided)*

- **The cat.** A small black cat with big lantern-yellow eyes suits the dark
  forest: a witch's cat the mad scientist has locked up in one of his lab cages
  (iron bars, a `LAB_GREEN` padlock). Drawn with flat primitives like the hero,
  through a render target (as `Player` and `MonsterCanvas` do) so it can squash,
  stretch and pounce as one piece.
- **The cage.** Place a locked cage with the cat inside on the road, deterministic
  per chunk like the flasks (`spot_for_chunk` in `src/game/world.rs`), so it can be
  tested the same way as `every_flask_is_reachable_and_on_the_road`. Flasks take the
  even chunks and monsters the odd ones, so the cage needs a chunk rule that
  doesn't collide with either (or a spot beside the road, see the open questions).
  Draw the cage with flat primitives (`gfx.rs`, `decor.rs`) and depth-sort it and the
  cat with the player through `Tall` / `tall_items`. While caged, the cat paces,
  paws at the bars and meows (a small "!" or "♪" bubble) when the player is near.
- **The puzzle modal.** Walking up to the cage and pressing E (`INTERACT_KEY`) opens
  an egui modal, built like `src/ui/task_panel.rs` (e.g. a new
  `src/ui/puzzle_panel.rs`). The puzzle is quick and solving it gives the player a
  key. One idea: a combination lock whose digits come from a small arithmetic clue,
  which keeps the learning theme and needs no API call. Give the puzzle its own
  state rather than reusing `TriggerState`, so it can't collide with an open flask
  task, and add it to the `busy` check in `Game::update` so the player stops while
  the modal is open.
- **Unlocking.** Using the key opens the cage with a short animation (the padlock
  pops, the door swings, the cat stretches and hops out), and the cat becomes the
  player's pet.
- **Following and "helping".** The cat trails the player with a little lag,
  using `world::clamp_to_path` so it stays on the road (and `World::clamp_blocked`,
  so it waits at a road blocker with the player), with idle and walk animations
  (tail swish, trot cycle, sitting to wash a paw when the player stands still, curling
  up asleep after a while). Its "help" is animation only: it purrs and rubs
  against the player's legs on a correct answer, flattens its ears and droops its
  tail on a wrong one, and arches its back and hisses at a monster in a battle,
  from beside the player's platform in the battle view (`Battle::draw_scene`). Keep
  the cat out of `Progression` so it can't affect gameplay (invariant 7).

**Open questions.**

- What does the cat look like: a black cat as suggested, or another colour, and
  does it have a name (picked by the player, or fixed)?
- Is the puzzle math (a clue that sets the lock's combination), or a non-math
  logic puzzle such as sliding tiles or matching a key's shape?
- Does the cage sit on the road and block it until opened, or beside it as an
  optional detour the player can skip?
- Is there one cat per run, or can more cages (and other pets) show up further
  along? How often?
- What happens on a failed or abandoned puzzle — retry right away, a cooldown like
  the flasks, or the cage is gone?
- Does the cat survive a curse game-over/restart (FR1)? Nothing is saved between
  runs today.
- Can the player pet the cat (E next to it) for a purr and a little heart, as a
  cosmetic touch?

**Acceptance criteria.**

- [ ] A locked cage with a cat appears on the road, placed deterministically and
  covered by a test, like the flasks.
- [ ] Pressing E at the cage opens a puzzle modal, and the player can't move while
  it's open.
- [ ] Solving the puzzle gives the player a key that opens the cage, with an
  unlock animation.
- [ ] The freed cat follows the player up the road, stays on the road, and
  draws in the right depth order with the player and scenery.
- [ ] The cat plays "helping" animations (at least on correct and wrong answers,
  and at a monster in a battle), and having the cat leaves XP, speed and answer
  checking unchanged.
- [ ] The puzzle works without an API key.

---

# Owner decisions

**Goal:** close the open questions FR5–FR15 asked. Those features are built
(see [Completed](#completed)), each on a provisional answer so the work could
land; this entry keeps the questions so they are answered, not settled silently.

## FR16. Confirm the provisional answers from FR1–FR3, FR5–FR15, FR20, FR24, FR26 and FR31

**Status.** **Waiting on the owner.** Every default below is live in the code.
Tick an item once the owner confirms it, or replace it with the owner's answer
and file the change as its own FR if it needs code.

**Depends on:** [] _(answers here may spawn small follow-up FRs)_

**Motivation.** FR5–FR15 were built in one pass, and FR1–FR3, FR20, FR24 and FR26 later on their own. Their open questions are the
owner's to answer, so each was given a reasonable, easily changed default and
recorded here.

**Decisions to confirm** (question, then the provisional answer in the code):

- [ ] **FR1** The owner chose the framing (both: the doctor's curse, felt as a
  fever), a health drain at full, a full pause while a question or battle is open,
  and a slow lift in the hub. Still provisional: does a correct flask cleanse some
  of it? *Yes, 0.25.* The numbers? *Full after 45 s standing still near the gate,
  rising up to ×1.5 by 2,000 m; each metre of new ground lifts 0.006; the hub lifts
  0.05/s; slows from 0.5 down to ×0.85 at full; at full, ½ heart every 3 s.*

- [ ] **FR2** Do road blockers replace flasks, or do both appear? *Both. Blockers
  replace the fern patch in some odd chunks; flasks keep the even ones.* How often?
  *Every sixth chunk from chunk 3 (`k % 6 == 3`), about every 80 m, the first at
  ~48 m, plus the Tide Titan at each 500 m mark, which now stands on the road too.*
  Does it scale with distance or level? *The spacing doesn't; which monster it is
  scales with distance (`MonsterKind::pick`), as for the fern patches.* How is the
  fight started? *E within 70 px of it; it never ambushes.* What stops the player?
  *A green rune ward across the road, 44 px in front of the monster.*

- [ ] **FR3** Math only, or other school subjects too? *Math only, as the prompt
  asks today; other subjects need a new answer type (multiple choice), see
  ARCHITECTURE's known limits.* What does a wrong answer cost? *The monster's hit:
  its damage in half-hearts (1–2, more for a grown Puffspike), a stun from Jellyglow
  and Coco Knight, the hat from a Gull Bandit, and the streak.* One right answer or
  several? *Several: the monster's `hits` (1–4).*

- [ ] **FR5** Jump/roll while standing still? *Yes.* Roll as a dodge later? *Not now;
  the roll has a defined start/end (`Action::Roll { t }`) so one can hook in.*
- [ ] **FR6** Pause with a flask task open? *Allowed* (the problem stays readable).
- [ ] **FR7** First-version settings? *Show progress panel, show token panel, reduce
  flashing, controls reference.* Persist between runs? *No.* Difficulty override?
  *No, `progression.level` drives it.*
- [ ] **FR8** Move during the spawn flicker? *Yes.* Invulnerable while it plays?
  *Yes* (`Player::spawn_protected`).
- [ ] **FR9** Hero: refresh or new character? *Refresh* (bolder ink, bigger head and
  eyes, pouch). Pixel font? *No, egui default with chunkier frames.* Collectibles?
  *Filed as FR17, not built.*
- [ ] **FR10** Wild encounters vs FR2 blockers? *Both coexist; see FR2 above.*
  Run from a battle? *No.* Catch/befriend? *No.* How often? *~70% of odd road
  chunks; flasks take the even ones.* Roster? *All 7 shipped: Pinchling,
  Jellyglow, Puffspike, Coco Knight, Gull Bandit, Dune Wyrm (rare from 400 m),
  Tide Titan (boss every 500 m).*
- [ ] **FR11** Knockback? *Yes, 22 px away from the source when it has one.*
  Invulnerability window? *1.0 s* (`HURT_INVULN_S`).
- [ ] **FR12** What a stun does? *Can't move and can't submit an answer until it
  ends.* Stacks with damage? *Yes, with the normal invulnerability window.*
- [ ] **FR13** Hearts or number? *5 hearts in half-hearts.* Sources/heals? *Monster
  hits (1–2 half-hearts; Puffspike grows); correct flask heals ½ heart; level-up
  refills.* Wrong **flask** answer costs HP? *Yes, 1 heart.*
- [ ] **FR14** Progression on death? *Everything resets.* Same world each run? *Yes
  (hash-seeded).* Persist a best distance? *No, session only.*
- [ ] **FR20** Do arrows hit anything on the road (monsters in the ferns, flasks)?
  *No, cosmetic like jump/roll (invariant 7): they arc, stick and fade.* Key? *F.*
  Limited ammo? *No.* In a battle? *A right answer plays as a bow shot instead of the
  old lunge; the hit is still decided by the answer alone.*
- [ ] **FR15** Hub size/contents? *~1.3 screens of scenery plus Professor Pebble's
  tutorial.* Walk back into the hub? *Yes.* Restart in the hub? *Yes.*
- [ ] **FR24** The manor's movement? *Pinned to the screen over the lake, drifting
  with the path's meander and the camera, so it's always ahead*, not a true parallax
  layer that scrolls past. How dark? *A vignette at half strength beyond 620 px from
  the player* (`NIGHT_STRENGTH` 0.5); *no brightness setting.* Lightning? *Every
  ~9 s behind the manor, a steady glow under reduce flashing.* Potion colours? *Green,
  violet or pink, by position.* Signs? *Hub sign "JEKO WOODS"; the gate keeps "THE
  ROAD".* The hero and monsters? *Unchanged, filed as FR25.* The manor's name?
  *None shown yet.*
- [ ] **FR26** Does a roll charge come back before all 3 are spent? *Yes, one per
  1.0 s without rolling; rolling back to back empties them, then a 1.5 s lockout
  refills all 3.* How long is a full sprint, and how fast does it return? *3 s;
  refills from empty in 4 s, starting 0.5 s after you stop.* What does winded do?
  *No sprint until stamina is back to 35%; walking unaffected.* Jump and shoot on
  the bar too? *No, they stay unlimited.* When is the bar shown? *Always on the
  road; hidden in a battle, while the dialogue box is open, and after death.* Do
  skills recover while held at a flask or in a battle? *Yes.*
- [ ] **FR31** Does the badge start as a circle? *Yes: it pops in as a circle with the
  goalpost's icon, unrolls into a pill with the name and metres, shows "Now!" on
  arrival, then closes back to a circle before the next one; a ring around the circle
  fills as you close in and pulses under 10 m.* Which goalposts show? *Stones every
  100 m (not the 50 m ones), flasks, fern patches, road blockers and the Tide Titan,
  not the hub; the nearest wins unless a bigger one (Titan, then blocker, milestone,
  patch, flask) is within 25 m of being as near.* One at a time or a queue? *One.*
  Does "Show progress panel" hide it? *Yes (`show_hud`); it's also hidden in a battle,
  a dialogue and while a flask's problem is open.* Keep the distance line in the
  Progress panel? *Yes.* The toast? *Steps down below the badge while it's up.*

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
deterministically like flasks (`spot_for_chunk`), spin and bob them, collect on
touch with a small sparkle (reuse `gfx::item_get`'s burst), and count them in the
HUD. Give them a use (e.g. a shop in the FR15 hub, or extra hearts).

**Open questions.**

- What are shells for: score only, a shop, healing?
- Do they reset on death (FR14)?
- *(raised by FR24)* Shells are a beach idea and the beach is gone. What should the
  coin be in the forest: acorns, gears and bolts from the doctor's lab, or glowing
  mushrooms? (The entry keeps the word "shells" until the owner picks.)

**Acceptance criteria.**

- [ ] Shells appear on the road deterministically, covered by a test.
- [ ] Walking over one collects it with feedback, and the HUD shows the count.
- [ ] Shells have the agreed use.

---

# Live content — generated monsters & lessons

**Goal:** use Cerebras' fast inference (Kimi K3.1, `CEREBRAS_MODEL`) for more than
one question at a time. While the player walks, a background worker writes **new
monsters** and **education content** ahead of need, and the running game picks them
up through **hot reload**. Today the model is used once per flask or battle turn,
in the foreground: the player stands still while it answers (`Game::update`'s busy
check). Every monster is Rust code (`MonsterKind`, `draw_pinchling` … in
`src/game/monster.rs`). FR21 makes monsters data that can be generated and reloaded.
FR22 uses the same worker to generate lessons ahead of time, so questions are ready
when needed and teach more than a bare number. FR32 comes first and needs neither: it
requests questions before the player reaches a flask or monster and overlaps them with
the battle's own animations. It also sets one priority order for every model call, so
the studio's background work never delays a question.

**Design in one picture** *(suggested, not decided)*:

```
                       ┌──────────── content::Studio (one long-lived thread) ───────────┐
 Game::update ──jobs──▶│ queue: GenMonster{tier,theme} · GenLessons{topic,level,n} · … │
  (look-ahead:         │ runs only when no live question is Loading (live always wins)   │
   zones/levels the    │ CerebrasClient::request_raw_blocking ─▶ parse ─▶ validate/clamp │
   player will reach)  └───────────────────────────────┬───────────────────────────────┘
                                                       │ atomic write (tmp + rename)
                                                       ▼
                            content dir: monsters/<id>.json · lessons/<topic>-<lvl>.json
                                                       │ ◀── also hand-edited by a person
                                    content::Watcher (polls mtimes ~1 s, own thread)
                                                       │ mpsc: ContentEvent::{Upserted, Removed, Invalid}
                                                       ▼
 Game::update drains with try_recv ─▶ Roster (Arc<MonsterSpec>) · LessonPool ─▶ encounters, battles, flasks
```

Rules this design keeps: the frame never waits (invariant 1), and the player
doesn't either (FR32: everything is ready before the encounter, and the studio takes
the lowest lane). Everything falls back
to the built-in roster and `assets/tasks_fallback.json` (invariant 2). The model never
sets a gameplay number directly (invariant 6). `content` stays plain Rust with no
macroquad or egui, and only `game` draws a spec (invariant 10). One part relaxes
invariant 3 and needs the owner's decision (see FR21's open questions).

## FR32. No waiting at an encounter: get questions and monsters ready before the player arrives

**Status.** **Milestones A, B and the soft deadline of C built (2026-11-21); the
shared priority lanes and the monster stock wait on FR21.** Done: `llm/buffer.rs`
(`QuestionBuffer`, the pure `plan_prefetch`, `WaitStats`), `game/prefetch.rs`
(`Prefetch`, `demand`, the limits and `JEKO_PREFETCH` / `JEKO_PREFETCH_RPM`),
`world::upcoming_in` and `World::upcoming`, `Game::request_task` taking from the buffer
first, the battle's first question asked in `start_battle` and each later one in
`battle_answer` (the "next" slot, shown when the phase reaches `Question`), the soft
deadlines (1.5 s battle, 2.5 s flask) that keep the late answer, `RequestKind` tags on
every call, the token panel's readout, and the `JEKO_DEV=fakemodel:<ms>` hook. Checked
by screenshot with `fakemodel`: a flask opens `Active` from the buffer on the frame E
is pressed, the panel shows "ready on arrival 1/1 · longest wait 0.0 s" and the
per-kind counts, and a 3 s model in a battle trips the deadline (bank question after
1.5 s, the late answer lands in the buffer).

**Next step:** the lanes. Once FR21's `Studio` exists, move prefetches onto its
thread and give every call one priority (live question > prefetch > villain > studio).
The monster stock lands with FR21 milestone C.

Provisional choices for the owner (change freely):

- *Level-ups:* take the exact difficulty, else one below, else ask live.
- *Across runs:* the buffer survives `reset()`.
- *Wasted calls:* about 1.5× the calls is accepted; `JEKO_PREFETCH_RPM` (20) bounds it.
- *Soft deadline:* 1.5 s in a battle and 2.5 s at a flask; a slow model gets a bank question.
- *Invariant 11:* not added. The doc states the rule in §5; promoting it is the owner's call.

**Depends on:** [] _(the base for FR21's studio and FR22's lesson packs, which feed
the same buffer; can reuse FR31's `goalposts_in_chunk` scan; warms
FR2's road blockers once they exist)_

**Motivation.** The game exists so that a fresh problem shows up without breaking the
walk (README, and §5 of the architecture doc), but every question is requested only
at the moment it's needed, so the player sees the whole round trip each time. From
the code as it stands:

| Where | When the request starts | What hides it | What the player sees |
|---|---|---|---|
| A battle's first question | `update_battle`, at `Enter` → `Intro` | `INTRO_S`, 0.8 s | A spinner if the model takes longer than 0.8 s |
| Every later battle question | `update_battle`, when `PlayerAttack` / `MonsterAttack` *ends* | Nothing: the 0.8 s / 0.9 s attack plays before the request starts | The full round trip, every turn |
| A flask | `open_spot`, when E is pressed | Nothing | The player frozen by the busy check, "Asking the tutor…" |
| A slow or failed call | `request_raw_blocking`'s 8 s timeout | Nothing | Up to 8 s of spinner before the fallback bank steps in |

Encounter zones and flasks are pure functions of the chunk index (invariant 3), so
the game knows where every one is, and the monster in it, long before the player
gets there. The waits come from the timing, not from the model being slow.

**Proposed approach.** *(suggested, not decided)*

- **A question buffer on `Game`.** `QuestionBuffer` (new `game/prefetch.rs`, or
  plain Rust in `llm` if it holds no game types) keeps a few ready `MathTask`s per
  difficulty 1–5, plus the prefetch receivers in flight. `Game::request_task(bias)`
  stays the only door (invariant 6): it takes a buffered task at that difficulty
  and returns `Active` on the same frame. Otherwise it makes a live request as today,
  and with no key it uses the fallback bank as today (invariant 2). Prefetching only
  fills the buffer. It never starts a question.
- **Overlap the battle's own animations** (milestone A, the cheapest win). Request
  the first question in `start_battle`, not at `Enter` → `Intro`, which gives it
  1.8 s of cover instead of 0.8 s. Request the next one in `battle_answer`, when the
  answer is submitted, not when the attack ends, so the request runs during the
  0.8–0.9 s attack. Skip it when that answer lands the last hit.
  The arrival waits in a "next" slot until the phase reaches `Question`.
- **Look ahead on the road** (milestone B). Each road frame, after
  `world.ensure_generated`, scan what's coming. A pure
  `world::upcoming(y, range_px) -> Vec<Upcoming { kind, y, difficulty_bias }>`
  could share its chunk scan with FR31's built `goalposts_in_chunk` / `World::next_goalpost`. For each uncleared zone
  within about 1.5 screens, keep its monster's `hits` worth of questions buffered
  at `level + bias`, plus one spare for a miss. For each fresh flask in range, keep
  one at `level`. A pure, unit-tested `plan_prefetch(upcoming, buffered, in_flight,
  caps) -> Vec<difficulty>` decides what to ask for, so the policy is testable
  without a network.
- **Limits.** At most 2 prefetches in flight, a per-minute cap
  (`JEKO_PREFETCH_RPM`, default 20), at most 3 buffered per difficulty, and
  `JEKO_PREFETCH=off` to turn it off. No new requests start while paused, but
  receivers are still polled, as `trigger_state` is. A live request beats a
  prefetch: none starts while `trigger_state` is `Loading`.
- **A soft deadline.** If a live request isn't back within `SOFT_DEADLINE_S`
  (about 1.5 s in a battle, 2.5 s at a flask), show a fallback-bank question
  straight away. Keep the late live answer in the buffer for next time instead of
  dropping it. The hard 8 s timeout stays as the backstop.
- **Monsters ready before their zone.** *(builds on FR21 milestones B and C)* A
  zone's species is bound when its chunk is generated, which is already one chunk
  above the top of the screen, and only to specs that are already in the `roster`.
  An encounter never waits on a `GenMonster` job. The studio's job is to keep a
  stock of unused species per tier ahead of the tiers the next zones need, and a
  zone with nothing in stock gets a built-in, as today. A parsed spec draws on the
  frame it's bound (`monster_rig`), so nothing loads at the encounter.
- **One priority order for every model call.** Live question > question prefetch >
  villain lines (FR23) > studio jobs (FR21's monsters, FR22's packs). It replaces
  each FR's own "live questions always win" check with one rule. Until FR21's
  `Studio` exists, this is just the in-flight check above. Once it does, the studio's
  queue takes the lower lanes, and prefetches can move onto its thread. That also
  settles the known limit "one thread and one HTTP client per request", which says
  to revisit it once requests prefetch.
- **Make the wait measurable.** Record where each shown question came from
  (`buffer | live | fallback | deadline`) and how long the player waited for it.
  The token panel shows something like "ready on arrival 18/20 · longest wait
  0.3 s". Tag prefetch requests in `RequestUsage`, the same kind tag FR21 and FR23
  propose. A dev hook `JEKO_DEV=fakemodel:<ms>` answers from the fallback bank after
  a delay on a thread, so the buffer, the deadline and the panel can be tested and
  screenshotted with no key.
- **When a wait still happens, keep it in the world.** In a battle, swap the spinner
  for the monster taunting or winding up, with "The Bramblehog is thinking…" on
  the nameplate (cosmetic, invariant 7). At a flask, the flask bubbles.

**Milestones** (each lands on its own):

1. **A: overlap the battle.** Request at `start_battle` and at `battle_answer`, the
   "next" slot, and the wait measurement and `fakemodel` hook. No look-ahead and no
   new limits.
2. **B: road look-ahead.** `QuestionBuffer`, `world::upcoming`, `plan_prefetch`
   with tests, the limits, and flasks opening from the buffer.
3. **C: the soft deadline and lanes.** The deadline swap and keeping late answers.
   The shared priority order lands when FR21's studio does, and the monster stock
   lands with FR21 milestone C.

**Open questions.**

- **Wasted calls.** Look-ahead asks for questions the player may never reach, for
  example when they die first or skip a flask. Is about 1.5× the calls acceptable,
  and what RPM and session token budget are fine?
- **The soft deadline.** It means a slow model gets a bank question instead of a
  fresh one. Is that the right trade, and are 1.5 s and 2.5 s right?
- **Level-ups.** A question buffered at difficulty 2 is one level too easy after a
  level-up. Use it anyway, drop it, or keep it for a lower-bias monster?
  Provisional: take the exact difficulty, else one below, else ask live.
- **Across runs.** Does the buffer survive `reset()`, like the client and the
  roster? Provisional: yes. The questions aren't run state, and that makes the
  first battle of a new run instant.
- **An invariant?** Should "the player never waits on the network when a buffered
  or fallback question could be shown" become invariant 11, next to invariant 1's
  "the frame never waits"?

**Acceptance criteria.**

- [ ] In a battle with an API key, each question after the first is requested when
  the previous answer is submitted, and the first in `start_battle`. With
  `fakemodel:700` no spinner shows in a battle. *(Built and the first question checked:
  it arrives ready. The later ones aren't confirmed under `fakemodel`, because scripted
  runs step game time at 1/60 s per frame while frames run faster than that, so the
  0.8 s attack covers less than 0.8 s of the stand-in model's wall-clock delay. Check it
  in a real-time run.)*
- [x] With look-ahead on, a flask or zone the player walks up to normally opens
  `Active` on the same frame, as measured by the wait readout.
- [x] `request_task` is still the only way a question starts, and with no key, no
  network or `JEKO_PREFETCH=off` the game plays exactly as it does today.
- [x] `plan_prefetch` and `world::upcoming` are pure and unit-tested, including the
  caps and the hub (nothing is prefetched for chunk `k < 1`).
- [x] A live call slower than the soft deadline is replaced by a bank question, and
  its late answer is kept for later.
- [ ] Once FR21 milestone C lands, a zone's species is bound off screen from the
  loaded roster only, and no encounter waits on a generation job.
- [ ] `docs/ARCHITECTURE.md` is updated: §5 (the buffer before the live call, the
  deadline), §4 (where `update_battle` and the road frame request), §11 (the env
  vars and the `fakemodel` hook), §13 (the one-thread-per-request limit), and
  §12 if the owner adds an invariant. *(Done for A, B and the deadline; the lanes
  and the monster stock remain.)*

---

## FR21. Generated monsters, hot-reloaded into the running game

**Status.** **Milestones A and B built (2026-11-21); C not started.** A: the
`content` module (`MonsterSpec`, `parse`/validation, the tier table, `Roster`,
`ContentEvent`), the polling watcher, `game/monster_rig.rs`, `Species` on `Battle`, the
live-swap rule, the `battle:<id>` dev event, four hand-written specs. B: the `Studio`
(`content/studio.rs`: one thread, a job queue, the `live_busy` gate that keeps it behind
live questions and look-ahead, `JEKO_STUDIO_RPM` and `JEKO_STUDIO_TOKENS`),
`content/prompts.rs` (the kid-safe, original-creature prompt with a shipped spec as the
shape), `finish_monster` (forces the tier, limits the special, vets the text, runs
`parse`, writes atomically as `monsters/gen-<name>.json`), `Game::tick_studio` (keeps 3
unseen generated monsters ready for the tier ahead), `RequestKind::Monster` tags, the
token panel's studio line and per-kind split, the `genmonster` dev event, and the
stand-in model writing made-up creatures under `JEKO_DEV=fakemodel`. Checked end to end
offline: the studio wrote three `gen-*` files, the watcher loaded them, and a generated
creature fought in a battle with the right nameplate and blurb.

Built differently from the sketch: the variant is `Species::Spec` (it covers
hand-written and generated specs alike), not `Generated`. `EncounterZone` still holds a
`MonsterKind`, because putting specs in zones is Milestone C and waits on the invariant 3
question. `MonsterInfo` lives in `content::monster_spec` so that `content` stays free of
`game`. Generated ids are `gen-<name>`, which is how the game and `.gitignore` tell them
from hand-written ones.

Provisional choices for the owner (change freely):

- *Content dir:* `JEKO_CONTENT_DIR`, else `./content`, else the build checkout's
  `content/`. Hand-written specs are committed. **Generated ones are a per-machine
  cache:** they persist in the dir between runs and `gen-*.json` is gitignored; rename one
  to keep it for committing.
- *Specials:* hand-written specs may use all four. **Generated ones may use `none` and
  `grows` only**; the studio turns anything else into `none`.
- *Moderation:* the kid-safe, original-creature prompt, a whole-word blocklist (other
  people's characters and anything not for children) on name, blurb, fact and topic, and
  the schema. No human approval step (`approved` flag) yet.
- *Studio model:* it uses `CEREBRAS_MODEL` like everything else; no `JEKO_STUDIO_MODEL`.
- *Limits:* 6 requests a minute, 60,000 tokens a session, one in flight, one job queued
  at a time.
- *Invalid-file toast:* shown always. *Tier table:* tier 1 = 1 hit/1 dmg/0 m/bias 0/15 XP,
  tier 2 = 2/1/50 m/0/20, tier 3 = 3/2/140 m/1/25, tier 4 = 3/2/400 m/2/30; stun is 2.0 s.
  Never a boss. *Fit:* a file's `id` must match its file name, or it's rejected.

**Next step:** Milestone C. It puts species in zones, which relaxes invariant 3, so ask
the owner first (see the open questions). Not built: the `reload` dev event, and
`gen_content --monsters`. FR32's lanes can now move look-ahead onto one shared queue.

**Depends on:** [] _(FR22 builds on its `Studio` and watcher; generated monsters fill FR10's encounter zones and, later, FR2's road blockers; FR32 sets the studio's priority lane and the rule that zones bind only ready species)_

**Motivation.** Seven hand-drawn monsters get repetitive a few hundred metres up the
road. Cerebras is fast enough to design a new creature in the background while the
player is still walking towards its zone. Hot reload is also a tool in its own right:
a person can edit a monster's JSON while a battle is open and watch it change, with
no rebuild. Today the assets are `include_str!`'d, so any change means a recompile.

**Proposed approach.** *(suggested, not decided)*

- **A declarative monster: `MonsterSpec`.** The model can't and mustn't write Rust,
  so a monster becomes data: a rig of parts drawn with the `gfx` vocabulary. New
  module `src/content/monster_spec.rs` (serde, no macroquad), roughly:
  - `id`, `name` (≤ 16 chars), `blurb` (one line for the intro), `fact` (a true
    real-world fact for FR22's field guide), `theme` (`{subject, topic}`, e.g.
    `{"math", "fractions"}`, which steers its questions in FR22).
  - `body`: `{shape: blob|shell|dome|segmented|wave|spiky_ball, w, h}`.
    `palette`: `{main, accent, belly}` as hex colours.
  - `eyes`: `{count: 1..=3, size, stalks: bool, look: cute|grumpy|sleepy}`.
  - `parts`: up to 6 of `{kind: claw|tentacle|spike|fin|wing|horn|leg|tail, count, size, side}`.
  - `accessory`: `none|crown|mask|helmet|bow_tie|leaf`.
  - `idle`: `bob|scuttle|float|sway|pulse`. `attack`: `lunge|zap|spin|slam|spit`.
  - `tier: 1..=4` and `special: none|stun|grows|steals_hat`. These are the existing
    mechanics only (FR10/FR12). The model can't make up a new one.
- **Stats come from the game, not the model.** `MonsterSpec::info()` maps
  `(tier, special)` to a `MonsterInfo` through a fixed table that matches the shipped
  roster's bands: tier 1 plays like a Pinchling, tier 4 like a Dune Wyrm. Bosses stay
  hand-made (Tide Titan). `min_m` comes from the tier as well. Out-of-range values are
  clamped, unknown enum strings are rejected, and the palette is lightened or
  darkened until it reads against `INK`. Validation is a pure function, unit-tested
  with hostile inputs.
- **One renderer for every spec.** Add `src/game/monster_rig.rs`: `draw_spec(o, spec, t,
  atk, hurt, grown)` is called from inside `monster::draw`, so the spec gets the
  existing `MonsterCanvas`, `Pose`, hurt flash, faint fade and reduce-flashing
  handling for free. Turn the monster identity into
  `enum Species { Builtin(MonsterKind), Generated(Arc<MonsterSpec>) }` and carry it
  on `Battle` and `EncounterZone` instead of `MonsterKind`.
- **The content dir and hot reload.** `JEKO_CONTENT_DIR`, defaulting to
  `content/` next to the binary. Gitignored or committed, per the open question.
  It holds `monsters/<id>.json`. `content::Watcher` runs on its own thread, polls the
  directory's mtimes about once a second (no new dependency; the `notify` crate would
  be the upgrade), parses and validates each changed file, and sends `ContentEvent`s
  over `mpsc`. `Game::update` drains them with `try_recv` into a `Roster`
  (`HashMap<id, Arc<MonsterSpec>>`), in the same place it polls `trigger_state`.
  A file that fails validation keeps the last good version and raises a toast in dev
  mode. **Live-swap rule:** art fields apply at once, even mid-battle, which is safe
  because art is cosmetic (invariant 7). `tier` and `special` apply from the
  monster's next battle, so a fight in progress never changes rules partway.
- **The `Studio` worker** (`src/content/studio.rs`). One long-lived thread holds a
  `CerebrasClient` clone and a job queue. That also covers the known limit "one thread
  and one HTTP client per request". A `GenMonster { tier, theme, avoid_names }` job
  sends a JSON-only prompt (new `content::prompts`, same style as
  `task_gen::build_prompt`). The prompt asks for an original forest creature for kids,
  in our art vocabulary, and explicitly not a character from any existing game or
  show. The worker parses the reply with `extract_json_object`, validates it, and
  writes `monsters/<id>.json` atomically. The watcher then loads it like any other
  file, so a generated monster and a hand-written one take the same path into the game.
  - **Live questions always win.** The studio runs in the lowest of FR32's lanes
    (live question > question prefetch > villain > studio), so it sleeps while a
    live question is `Loading` or a prefetch is due. It keeps at most one request in flight, under a per-minute cap
    (`JEKO_STUDIO_RPM`, default 6) and a session token budget.
  - **Look-ahead.** Each frame, `Game` checks how many unused generated species are
    eligible for the next few encounter zones, using FR32's `world::upcoming`. Below
    a small target (e.g. 3), it enqueues a `GenMonster` for the tier those zones need.
    The stock has to be built well before a zone, because a zone never waits for one.
  - `JEKO_STUDIO=off` disables it. No API key means it never starts.
- **How generated monsters reach the road** (keeps invariant 3 as far as possible).
  `encounter_for_chunk` stays pure, so where zones are and their `roll` don't change.
  When a zone's chunk is generated (one chunk above the top of the screen, so off
  screen), `Game` decides its species once, from specs already in the roster
  (FR32: an encounter never waits on a generation job):
  `MonsterKind::pick` as today, or, with a share set by `GENERATED_SHARE` (e.g. 40%),
  a generated species of the right tier chosen by hashing the zone's roll over
  the roster sorted by id. The choice is stored on the zone, and chunks are never
  dropped, so walking back shows the same monster. With an empty roster, the game
  behaves exactly as it does today.
- **Visibility.** Tag each `RequestUsage` with a kind (`question | monster | lesson`).
  The token panel then splits throughput by kind and shows the studio's queue depth.
  The battle intro shows a small "new species!" banner the first time a generated
  monster is met.
- **Tooling.** `gen_content --monsters N` batch-generates through the same studio
  code path, offline, so a curated set can be reviewed and committed. New dev
  events: `genmonster` (enqueue a job), `battle:<id>` (fight a generated species),
  `reload` (force a rescan).

**Milestones** (each one lands on its own):

1. **A: data-driven monsters, no LLM.** `MonsterSpec`, validation, `monster_rig`,
   `Species`, watcher and roster. Ship two or three hand-written specs in
   `content/monsters/`. Editing one while the game runs changes it on screen.
2. **B: the studio.** A background `GenMonster` job with the prompt, rate limit,
   look-ahead and token tagging.
3. **C: on the road.** Lazy species binding for zones, `GENERATED_SHARE`, the
   "new species!" banner, and the `gen_content --monsters` batch mode.

**Open questions.**

- **Invariant 3 (owner's call):** the binding above makes *which* monster stands in
  a zone depend on what's in the content dir, not only on the chunk index. Is that
  acceptable? Alternative: freeze the roster at `reset()`, so new species join only
  on the next run and a run stays reproducible given the content dir.
- Where does the content dir live, and is generated content committed (curated, like
  `assets/dialogue.json`) or per-machine cache only? Does it persist between runs?
  Today nothing persists (FR14/FR16).
- Can generated monsters use `stun` and `steals_hat`, or start with `none|grows` only?
- How much moderation is needed beyond a schema, a name blocklist and a kid-safe
  prompt? Should a person approve generated monsters before they appear (a
  `"approved": false` flag that `gen_content` flips)?
- Should the seven built-ins eventually move into specs too, or stay hand-coded as
  the "classic" set? They're more detailed than a parts rig can draw.
- Confirm the real Cerebras model slug: `config.rs` defaults to `kimi-k3.1`, which the
  architecture doc calls a placeholder. Should the studio use its own
  `JEKO_STUDIO_MODEL`, so a slower, higher-quality model can design creatures while
  the fast one keeps answering questions?

**Acceptance criteria.**

- [x] A monster defined only by a JSON spec in the content dir can be fought, with
  idle, intro, attack, hurt and faint poses, and honours reduce flashing.
  *(Milestone A; fought via `battle:<id>` until C puts specs on the road.)*
- [x] Editing that JSON while the game runs updates the monster within about 2 s,
  with no restart. An invalid edit keeps the last good version and doesn't crash.
- [x] Spec validation is a pure, unit-tested function: hostile input (huge sizes,
  unknown enums, invisible colours, long names) is clamped or rejected, and stats
  always come from the tier table.
- [x] With an API key, the studio generates new monsters in the background while
  the player walks. The frame never blocks (invariant 1), and a live question is
  never delayed by a studio request. *(Checked offline with the stand-in model: it
  holds its job while a question is loading, a test covers that. Not yet watched with the
  real model during play.)*
- [ ] Generated monsters appear in encounter zones. Revisiting a zone shows the same
  monster, and zone placement is still covered by the existing determinism tests.
  A zone's species is bound off screen from the loaded roster, so no battle waits on
  a `GenMonster` job (FR32).
- [x] With no key, no network or an empty content dir, the game plays exactly as it
  does today. *(No key: no studio starts. An empty dir: the roster is empty and zones are unchanged.)*
- [x] The token panel splits usage by request kind.
- [ ] `docs/ARCHITECTURE.md` (new `content` module, the watcher in the frame,
  `Species`, env vars and dev events, invariant 3 as decided) and the README layout
  are updated. *(Done for Milestone A; the studio, zones and invariant 3 remain.)*

---

## FR22. Background-generated education content (lesson packs, explanations, field guide)

**Status.** **Not started.** Designed only.

**Depends on:** [FR21, FR32] _(reuses FR21's `Studio`, content dir and watcher, and
fills FR32's `QuestionBuffer` instead of keeping a pool of its own; the themed questions need FR21's `MonsterSpec.theme`; touches FR3's open question on non-math subjects)_

**Motivation.** Each question today is a single live call, made while the player
stands still and waits. All it teaches is a hint and right or wrong. Fast inference
lets the game generate **ahead of time** and **in bulk**. The flask can then open
instantly, a wrong answer can come with a worked explanation, and what the player
gets wrong can steer what gets generated next.

**Proposed approach.** *(suggested, not decided)*

- **Lesson packs, prefetched.** A `GenLessons { topic, level, n }` job asks for a
  batch (e.g. 8) of problems for one topic and difficulty. The studio writes them to
  `lessons/<topic>-<level>.json`, and the watcher loads them into FR32's
  `QuestionBuffer`, keyed by topic as well as difficulty. `Game::request_task` (still
  the only door, invariant 6) already takes from that buffer first, then asks live,
  then uses the fallback bank (FR32). Packs add bulk, verified answers, explanations
  and topics. FR32's look-ahead already makes the single-question version instant, so
  the studio refills a pack when FR32's planner reports a topic running low.
- **Answers are checked before a child sees them.** Each generated problem also
  carries an `expression` (e.g. `"(12 + 8) / 4"`). A tiny pure evaluator in
  `content` (numbers, `+ − × ÷`, parentheses) computes it, and the problem is dropped
  if it doesn't equal `answer`. A wrong answer key is the worst failure an
  education game can have. Unit-test the evaluator.
- **Explanations.** Add `explanation: Vec<String>` (2–4 short steps) to `MathTask`
  (`#[serde(default)]`, so the fallback bank and live prompt keep working). On a wrong
  answer, `task_panel` shows the steps before the cooldown ends. In a battle, they
  show during `MonsterAttack`.
- **Themed battles.** A generated monster's `theme` (FR21) goes into the question
  prompt. A fraction-loving jellyfish asks about fractions, and a tide monster asks
  about tide times, still with numeric answers.
- **Adaptive practice.** `Progression` records accuracy per `topic` as plain counters,
  with no rendering. The studio's look-ahead tilts lesson jobs towards the player's
  weakest topics. A missed problem comes back later as a *similar* one: a
  `GenLessons` job with `like: <problem>`.
- **The field guide.** Beating a monster unlocks its card: name, blurb and `fact`.
  The prompt asks for well-known, conservative facts about the real animal or
  phenomenon it's based on, such as real hermit crabs trading shells. A new
  `ui/field_guide_panel.rs` (a view, invariant 5) is opened from settings or a key.
- **Pebble's tips.** A `GenDialogue { npc, context }` job (the runtime version of
  `gen_content`) writes tips for Professor Pebble about topics the player missed this
  run, hot-reloaded into the dialogue bank.

**Open questions.**

- Math only, or other subjects (science facts in the field guide, reading, geography)?
  Anything that isn't a number needs a new answer type such as multiple choice
  (FR3's open question and *Known limits* in the architecture doc).
- Show the explanation on every wrong answer, or only after two misses on the same topic?
- Are field-guide facts shown as generated, or only after a person reviews them
  (committed via `gen_content`)?
- Does per-topic accuracy persist between runs? It's the first piece of learner data
  the game would keep, which needs a privacy decision as well as a code one.

**Acceptance criteria.**

- [ ] With an API key, most flasks open from a prefetched pack in FR32's buffer, with
  no visible wait. FR32's single prefetches, the live call and the fallback bank still
  cover an empty pack or no key.
- [ ] Every generated problem's `answer` is verified by evaluating its `expression`,
  and failures are dropped (unit-tested).
- [ ] A wrong answer shows a short worked explanation, and the fallback bank still
  loads without that field.
- [ ] A generated monster's battle asks questions on its theme.
- [ ] Per-topic accuracy steers which lessons the studio generates next.
- [ ] Beating a monster adds its card to a field-guide panel.
- [ ] `docs/ARCHITECTURE.md` §5 (the pool before the live call, explanations,
  verification), §10 (the new panel) and §11 (the content dir) are updated.

---

# World — the map and the destination

**Goal:** give the road somewhere to go and a way to see how far you've come.
Today the road is endless (`docs/ARCHITECTURE.md` *World model*) and nothing shows
the player where they've been. The owner wants a map that fills in as the player
explores, and a destination at the end: a village town with a castle, where the mad
scientist lives.

## FR27. A discovery map on M, and a village with the mad scientist's castle at the end of the road

**Status.** **Not started.** Requested by the owner; the approach is suggested only.

**Depends on:** [] _(pairs with FR23: the castle is the mad scientist's home and the
natural place for a final showdown; touches FR24's manor, see open questions)_

**Motivation.** Walking north with no picture of the journey makes every 100 m feel
the same. A map that reveals the road as the player walks it rewards exploring, and a
goal marked on it (the castle, still in fog) turns "walk forever" into "get to the
castle". The owner also wants to grow the map over time, adding places along the
road as the game develops, so the map has to be built from data that's easy to
extend, not a hand-drawn picture.

**What the owner asked for.**

- Pressing **M** opens a map of what the player has discovered so far.
- The map is dynamic: it fills in as the player goes up the road, and the owner can
  keep adding to the world and have the map pick it up.
- The end goal is a **village town** with a **castle** at the far end. The castle is
  where the mad scientist lives.

**Proposed approach.** *(suggested, not decided)*

- **Discovery state.** A small `Discovery` on `Game` (new `game/map.rs`) records the
  furthest north `y` reached and the landmarks seen (flasks solved, monsters beaten,
  bosses, regions entered). It resets with `Game::reset()` like the rest of the run
  (FR14), unless persistence lands.
- **The map draws from the world functions, not a stored map.** Because the world is
  a pure function of `y` and the chunk index (invariant 3), the map can sample
  `world::path_center` between the hub and the furthest point reached and place
  icons with `spot_for_chunk` / `encounter_for_chunk`: the hub camp, the gate, a
  marker every 50 m (the milestones), solved and unsolved flasks, beaten monsters,
  Tide Titan marks, and a "you are here" marker. Anything north of the furthest point
  is fog.
- **Regions.** A table of regions along the road, keyed by distance (for example:
  the dark forest, then the village outskirts, then the town, then the castle), kept
  as data (`assets/regions.json` or a const table in `world.rs`) so adding a region
  is one entry. Each region gives a name, a map colour and icon, and what
  `decor::generate_chunk` places there. Entering one shows its name as a banner and
  labels it on the map. This is how the owner grows the map over time.
- **The destination.** The last region is the village and the castle. The road
  ends at the castle gate, so the road is no longer endless. The castle shows on the
  map from the start as a silhouette in the fog with a "?", so the player always
  knows where they're headed.
- **The panel.** `ui/map_panel.rs`, an egui window in the parchment theme. It reads
  `Discovery` and the region table and returns events only (invariant 5). M toggles
  it and Escape closes it. Whether the game pauses while it's open is an open
  question.

**Milestones (each lands on its own).**

1. **A: the map.** `Discovery`, the M key, the map panel with the discovered road,
   landmarks, fog and the player marker, all inside today's forest.
2. **B: regions.** The region table, region banners and labels on the map, and
   scenery chosen per region.
3. **C: the village and the castle.** The village and town regions, the castle at
   the end of the road, its silhouette on the map from the start, and what happens
   when the player arrives (see open questions; a showdown with the mad scientist
   would build on FR23 and FR2/FR3).

**Open questions.**

- **How far is the castle?** A fixed distance (say 2 000 m), or does it grow as the
  owner adds regions? What happens past it: the run ends with a win screen, the
  road continues, or a new loop starts?
- **The castle and FR24's manor.** The manor across the lake is already drawn as the
  mad scientist's home, pinned ahead on the screen. Does the castle replace it, does
  the manor become the castle seen from afar, or are they two places?
- **FR23's "never reached" villain.** FR23 keeps the doctor always out of reach. Does
  the player finally catch them at the castle?
- **Pause.** Does opening the map pause the game (like settings, FR7), or stay live
  so the player can glance at it while walking? Can it open during a flask problem
  or a battle?
- **How much detail?** Show every flask and monster zone, or only landmarks (hub,
  gate, milestones, bosses, regions)? Does it show unsolved flasks the player walked
  past?
- **Fog.** Is the castle the only thing visible in the fog, or are the region names
  hinted too?
- **Persistence.** Should discovery carry over between runs (a map that stays
  filled in), which needs the persistence that doesn't exist yet (*Known limits*)?
- **A minimap.** Is a small always-on minimap in a corner wanted too, or only the
  full map on M?

**Acceptance criteria.**

- [ ] Pressing M opens a map and pressing it again (or Escape) closes it. M is listed
  in the controls reference.
- [ ] The map shows the hub, the gate and the road the player has walked so far, with
  landmarks and a "you are here" marker. The road north of the furthest point
  reached is fogged.
- [ ] Walking further north fills in more of the map, the same way every run.
- [ ] Adding a region is one entry in the region table, and the map and the world
  both pick it up.
- [ ] The road ends at a village town with a castle at the far end, where the mad
  scientist lives. The castle is marked on the map as the goal before it's reached.
- [ ] The map changes no XP, HP, answers or battle results (invariant 7), and the
  panel keeps no gameplay state (invariant 5).
- [ ] A dev event (for example `map`) opens the map without the keyboard, and one
  (`tocastle`) snaps the player near the end, so both can be checked by screenshot.
- [ ] Tests cover `Discovery` updates and that regions and the castle sit at fixed
  distances (invariant 3).
- [ ] `docs/ARCHITECTURE.md` (the road is no longer endless, the new modules, the
  panel, the region table, the dev events, *Known limits*) and the README layout are
  updated.
- [ ] The castle, the village and region borders are goalposts for the tracker
  (FR31: a new `GoalKind` in `world::goalposts_in_chunk`).

---

# World — the road itself

**Goal:** make the road worth walking for its own sake. Today it's one cobbled lane
with an unbroken fence on both sides, a gentle sway, and nowhere to stop between
the hub and wherever the player dies. The owner wants places to rest along the way
(FR28) and a road that bends, narrows and is cluttered with things to walk around
(FR29).

## FR28. Safe stops along the road: a little house to rest at

**Status.** **Not started.** Requested by the owner; the approach is suggested only.
The owner's wording was "safe hubs like a quick mouse"; this entry reads it as a
small house or rest stop. Confirm that reading (see open questions).

**Depends on:** [] _(pairs with FR1, built: a safe stop is where the doctor's curse
gives the player a breather; FR27's map shows each stop as a landmark; FR17's shells could be spent
here)_

**Motivation.** Once the player leaves the hub there's no let-up: flasks, monsters
and the doctor's curse (FR1), which closes in whenever they stop moving, all the
way up. A safe stop every so often gives the run a
rhythm (push, rest, push), a place to heal before a Tide Titan, and a goal a little
way ahead that's closer than the castle.

**What the owner asked for.**

- Safe hubs as part of the road, which the player comes across as they walk.
- Something like a small house the player can stop at.

**Proposed approach.** *(suggested, not decided)*

- **Placement.** A pure function `world::safe_stop_for_chunk(k)`, like
  `spot_for_chunk` and `encounter_for_chunk` (invariant 3). A fixed spacing reads
  best, for example one stop every 250 m, put a chunk or two *before* each 500 m
  Tide Titan so the player can heal before the boss. A chunk with a stop gets no
  flask and no encounter zone, and the chunk either side gets no encounter zone.
- **The clearing.** The road widens into a small lantern-lit clearing at the stop.
  `walkable_x` and the fence ease out and back in around it the way they already do
  through the hub funnel, so the fence opens onto the clearing.
- **The house.** A hand-laid layout per stop (like `decor::hub_layout`, with a
  seeded variant so stops differ): a small cottage with lit windows, a bench, a
  lamppost or two, a well or a campfire. New `decor::Kind`s where needed (`Bench`,
  `Well`). Optionally a keeper NPC who talks through the dialogue bank
  (`assets/dialogue.json`), the way Professor Pebble does.
- **What makes it safe.** Inside a stop's span: no encounters spawn, the player
  can't be damaged, and the curse eases. `Curse::tick` already decides by the
  player's `y` (rising on the road, lifting at `HUB_LIFT_PER_S` in the hub), so a
  stop's span can either pause the curse or lift it the way the hub does. That
  makes a stop the one place up the road where standing still doesn't bring the
  fever on. Resting there heals:
  either a full `Health::refill` on arrival, or a slow heal while the player stands
  in the clearing (one door per rule, invariant 6).
- **Optional extras** (the owner picks): a checkpoint so dying restarts at the last
  stop reached instead of the hub (FR14's restart currently goes back to the start);
  a shop that spends shells (FR17); a "rest" prompt on E that shows the run stats so
  far.
- **Invariant 4** says the hub is the safe zone. This extends it: "nothing hostile
  spawns south of `ROAD_SOUTH_END` or inside a safe stop's span". Changing the
  invariant is the owner's call; record the answer in `docs/ARCHITECTURE.md`.

**Open questions.**

- **Is "a small house / rest stop" what was meant by "a quick mouse"?** Or was it
  something else, such as a mouse character who runs the stop?
- **How often?** Every 250 m, every 500 m before the Titan, or more often early on
  and less often further up?
- **What does resting give?** A full heal on arrival, a slow heal while inside, or a
  heal only once per stop per run?
- **Checkpoint?** Does dying send the player back to the last stop, or still to the
  hub?
- **What does a stop do to the curse?** Pause it, lift it like the hub
  (0.05/s, about 20 s from full to clear), or only slow its rise?
- **Can the player linger forever?** On the road the curse fills in 45 s of standing
  still (`FULL_AFTER_S`). If a stop pauses or lifts it, the player can stand there
  indefinitely, as they already can in the hub. Is that fine, or does the curse keep
  rising there, just slower, so the stop is a breather, not a hiding place?
- **A keeper?** Is there someone at each stop (one recurring character, or a
  different one each time), or is it an empty house?

**Acceptance criteria.**

- [ ] Safe stops appear along the road at fixed distances, the same every run.
- [ ] Each stop is a clearing off the road with a small house and props, and the
  fence and walkable width open onto it smoothly.
- [ ] No encounter zone or flask threatens the player inside a stop, and the curse
  behaves there as decided above (covered by a `game/curse.rs` test).
- [ ] Resting at a stop heals as decided above, through `Health`.
- [ ] Placement is covered by a test: stops are deterministic, on the road, never
  share a chunk with a flask or a zone, and never sit next to a zone.
- [ ] A dev event (for example `tostop`) snaps the player next to the first stop so
  it can be checked by screenshot.
- [ ] `docs/ARCHITECTURE.md` (*World model* table, invariant 4, the curse's rule
  at a stop, the new `decor` kinds, the dev event) is updated.
- [ ] A safe stop is a goalpost for the tracker (FR31: a new `GoalKind` in
  `world::goalposts_in_chunk`).

---

## FR29. A road that winds, a fence that breaks, and things in the way

**Status.** **Not started.** Requested by the owner; the approach is suggested only.

**Depends on:** [] _(touches FR28's clearings and FR27's map, which both read
`path_center`; FR29's obstacles can share the collision work with FR2's built
blockers, `World::clamp_blocked`)_

**Motivation.** The road today is almost a straight line north. `path_center` is two
slow sines with at most ±88 px of sway, `PATH_HALF_W` never changes, and the fence is
an unbroken row of posts every 84 px on both sides. After a minute every stretch
looks the same. A road that bends hard, narrows and widens, with a fence that's
broken in places and junk left on the cobbles, makes the walk feel like a journey
and gives the player something to steer around.

**What the owner asked for.**

- The road doesn't have to run so straight up the screen.
- The fence doesn't have to be so straight. Make it more dynamic.
- Objects on the road: broken-down cars, benches, and so on. Other ideas welcome.

**Proposed approach.** *(suggested, not decided)*

- **A road that bends.** Give `path_center` more range: larger, slower sweeps plus
  occasional sharp bends (a sum of sines with a seeded bend every few hundred
  metres). The catch is the camera. `View::new` only follows `y` and centres the
  960 px strip, so a road that swings more than about ±150 px walks off the screen.
  The camera has to follow the road's x too (ease `View.offset.x` towards
  `path_center` at the player's `y`), and the lake and woods (`bank_x`,
  `treeline_x`) already follow the path, so they come along. Keep the ramp in from
  straight near the gate so the hub is unchanged.
- **A road that changes width.** Make the half-width a function `path_half_w(y)`:
  narrow stretches between the trees, wide stretches, and the clearings for FR28.
  `walkable_x`, `clamp_to_path`, the cobbles and the fence all read it instead of the
  `PATH_HALF_W` constant.
- **A fence that isn't a ruler line.** In `World::posts`, per post and seeded with
  `gfx::hash01`: a small jitter in spacing and inset, posts that lean, a missing rail,
  a gap of two or three missing posts, a fallen post lying on the verge. Some
  stretches swap the iron fence for something else: a low stone wall, a broken
  wooden rail, a hedge, or no fence at all where the road crosses open ground. The
  fence stays visual: `clamp_to_path` is still the edge whether there's a post or not.
- **Things on the road.** A new per-chunk placement, `world::obstacles_for_chunk(k)`
  (invariant 3), of props that sit on the cobbles and that the player has to walk
  around. Ideas, to fit the dark forest:
  - a broken-down car or an abandoned carriage with a missing wheel (the owner's
    call which fits better; a rusted old car could be the mad scientist's)
  - a bench, a toppled lamppost, a fallen tree across half the road
  - crates and barrels from the doctor's lab, a spilled cart of pumpkins
  - a pothole or puddle (slows the player, no collision), a cracked stretch of
    cobbles
  - a short wooden bridge where a stream crosses, a stone well, a road sign at a
    bend
- **Collision.** Today `clamp_to_path` is the only movement constraint. Solid
  obstacles need a second one: a simple circle or box per obstacle, pushed out after
  the path clamp, also applied to knockback and rolls. Placement must always leave a
  gap wider than the player (say 60 px of clear cobbles) and never sit on a flask,
  an encounter zone or a FR28 stop.
- **More ideas (welcome, not required):** a fork where two roads split around an
  island of trees and rejoin; a stretch where the road runs along the lake shore with
  no fence; fog banks that thicken in hollows; a crow that flies off when the player
  gets close.

**Open questions.**

- **How wild can the bends get?** Gentle S-curves, or proper hairpins the camera has
  to swing round for? Does the camera following x bother anyone (motion), and should
  reduce-motion keep it gentler?
- **Cars or carriages?** The setting is a gothic forest. Do broken-down cars fit,
  or should the wrecks be carriages and carts?
- **Do obstacles block or just decorate?** Solid things to steer round, or
  props the player walks over? Can arrows (FR20) hit them?
- **Any that interact?** For example, sit on a bench to rest (overlaps FR28), or kick
  a barrel to find a shell (FR17).
- **Forks.** Wanted, or out of scope? A fork means two walkable lanes at one `y`,
  which `walkable_x` (one range per `y`) can't express today.

**Acceptance criteria.**

- [ ] The road visibly winds left and right, with some sharper bends, and the camera
  keeps the player and the road on screen at the default window size.
- [ ] The road's width varies along its length, and the walkable width, cobbles and
  fence all follow it.
- [ ] The fence varies: jittered and leaning posts, gaps, and at least one other
  edge style (wall, wooden rail, hedge or none).
- [ ] Obstacles such as a wreck (car or carriage), a bench and crates sit on the
  road, and solid ones can't be walked through.
- [ ] The road is always passable: a test walks every chunk and checks a clear gap
  past every obstacle, and that no obstacle covers a flask, a zone or a stop.
- [ ] Generation stays deterministic (same world every run), with a test.
- [ ] The hub and the gate look and play as they do today.
- [ ] `docs/ARCHITECTURE.md` (*World model*: path geometry, the camera, the new
  placement and collision; *Rendering*: the fence) is updated.
- [ ] Solid obstacles are goalposts for the tracker (FR31: a new `GoalKind` in
  `world::goalposts_in_chunk`).

---

# Story — the villain

**Goal:** give the endless road a face and a reason. Today nothing ties the
flasks, monsters and road together. A recurring antagonist who's always
just ahead, reacting to what the player does, makes the walk feel like a chase
and gives Cerebras a second live job: writing what the villain says and schemes,
in the moment.

## FR23. The ultimate villain: a mad scientist who's always up the road, live-scripted by Cerebras

**Status.** **Not started.** Designed only. Milestone A needs no API key, so start
there.

**Depends on:** [] _(stands alone; Milestone C pairs with FR21/FR22, where the
villain becomes the in-world face of the content `Studio`, and a final showdown
would build on FR2/FR3's blocking battles)_

**Motivation.** The player walks north forever with no one to chase. A mad
scientist who potters around at the top of the screen, always a little out of
reach, turns "walk up the road" into "catch the doctor". Because Cerebras answers
in well under a second, the villain can react **to this run**: the flask just
missed, the Puffspike just beaten, the 10-answer streak. Canned lines can't do
that. In the story, the monsters are the doctor's creations, which ties the FR10
roster (and FR21's generated monsters) to one cause.

### Character design *(suggested, not decided: name and lore are the owner's call)*

**Dr. Nix Nullwell** ("Dr. Nullwell"): a gangly, over-caffeinated inventor who
wants to turn every number in the woods into **zero**, so nobody ever has to do
sums again. Comic rather than scary. They brag about their inventions and fall
apart when the player gets things right. They're bad at maths themselves: their
chalkboard says `2 + 2 = 5`.

```
            ~~ ~ ~~            frizz puffs (grow when flustered)
          (( (  _  ) ))        bald dome, one cowlick
           \\( O=O )//         brass goggles, glowing green lenses
             \  ᵕ  /           long nose, toothy grin, waggling brows
          .---| ⋈ |---.        crooked bow tie
         /  [|::|::|]  \       long lab coat: scorch marks, patched pockets,
     🧪-o   |  ;;  |    o      test tubes and pencils in the breast pocket
            | /  \ |           rubber gloves, bubbling flask in one hand
            |/    \|           coat tails flap when walking
           _|_    _|_          big clompy boots
```

| Part | Look | Animates |
|---|---|---|
| Silhouette | About 1.25× the hero's height, hunched, oversized head, so they read as "the boss" from the top of the screen | Leans and bobs with the walk |
| Hair | Two big white frizz puffs over the ears, bald dome, one cowlick | Frizz grows and sparks when flustered |
| Goggles | Brass rims, round lenses with a soft green glow | Glow pulses slowly (steady under reduce flashing) |
| Face | Long nose, bushy brows, big grin; eyes are the lenses | Brows waggle while cackling or scheming |
| Coat | Cream, too long, scorch marks, mismatched patches, stained pockets | Tails flap with speed; puffs of soot on a fluster |
| Gloves and boots | Purple rubber gloves, clompy dark boots | Gloves wave while monologuing |
| Flask | Bubbling glass flask; the liquid's colour follows the current scheme | Bubbles rise; it fizzes over on a fluster |
| Contraption cart | A rolling lab bench with a small Tesla coil, a chalkboard and a crank | Sparks while tinkering (steady glow under reduce flashing) |

Palette: coat `#F2EEDF`, brass `#C89B3C`, lens glow `#7CFF6B`, gloves `#8E5BD6`,
outlines in `gfx::INK`, the shadow in `gfx::SHADOW`, so they sit in the existing
storybook-cartoon look. Drawn with flat primitives like everything else (no
textures), into a 2× render target like the player and monsters, so the
appear/vanish puff can fade them as one piece.

**Behaviours** (a closed enum, `VillainAction`): `Pace` (stroll across the top of
the road, coat flapping), `Tinker` (bent over the cart, wrench, sparks), `Brew`
(pour the flask, a puff of coloured smoke), `Cackle` (leans back, shoulders
shake, a "MWAHAHA!" word-burst), `Scribble` (writes a wrong sum on the chalkboard),
`Spy` (telescope pointed at the player), `Fluster` (frizz, fizzing flask: the
player got something right), `Gloat` (rubs gloves: the player got something wrong),
`Scurry` (dashes ahead pushing the cart when the player gains on them), and
`Appear`/`Vanish` (a soot puff).

### Proposed approach *(suggested, not decided)*

**Where they are and how they move.**

- New module `src/game/villain.rs` holds `Villain { pos, action, action_t, mood,
  bubble, … }` on `Game`, plus `Villain::update(dt, cam_y, player_pos, time)` and
  `draw`.
- **Always at the top of the road.** Each frame the target `y` is a band just
  below the top of the screen (`cam_y` minus the camera anchor plus roughly 60–110
  px), and the target `x` wanders along the road and the verge beside it with
  `world::path_center(y)` and `walkable_x(y)`. The doctor eases towards that
  target. When the camera moves fast (sprint), the ease speeds up and the action
  switches to `Scurry`, so the player can never catch them. They stay in view
  without ever being reachable.
- **Not a placed thing.** Their position comes from the camera and the game clock,
  not from the chunk index, so invariant 3 isn't touched and no world-generation
  test changes. Any wander jitter uses `gfx::hash01` on game time, so `JEKO_DEV`
  runs stay repeatable frame for frame.
- **Drawn in the depth-sorted pass** as a new `Tall::Villain` keyed by their ground
  `y`. They're always north of the player, so trees and cottages near the top of the
  screen overlap them properly. Under them goes a slight haze tint (a `gfx::mix`
  towards the night colour, `gfx::NIGHT`) so they read as "far up the road".
- **First appearance** is at the iron gate (`ROAD_START_Y`), with an `Appear`
  puff and an intro line. They aren't shown in the hub, which stays calm (invariant
  4). They're hidden during the battle view and Professor Pebble's dialogue, and
  they freeze with pause (invariant 8).
- **Speech bubble.** A world-anchored bubble above their head, drawn in `game` with
  a new `gfx::speech_bubble` (same parchment and dark-wood look as `gfx::key_prompt`),
  not an egui panel. That keeps `game` free of egui (invariant 10). One line at a
  time, about 4 s on screen, with a typewriter reveal reusing `Dialogue`'s pacing.
  Whether it hides with `show_hud` is an open question.

**What Cerebras generates in real time: a "beat".**

The game sends a short **cue** describing what just happened, and the model replies
with one JSON object that's validated before use:

```json
{
  "line": "Impossible! That was my FINEST fraction! Back to the lab!",
  "mood": "flustered",
  "action": "fluster",
  "scheme": null
}
```

- `line`: ≤ 90 chars, kid-safe. `mood`: `smug | gleeful | flustered | scheming |
  furious_but_funny`. `action`: one `VillainAction`. `scheme`: optional (Milestone
  C). Unknown enums become a default for the mood, an over-long line is cut at a
  word boundary, and a line that hits the blocklist is replaced from the fallback
  bank. Validation is a pure function in `llm`, unit-tested with hostile input.
- **Cues** (`VillainCue`, built in `Game` from events that already exist):
  `FirstSighting`, `Milestone { meters }` (every 100 m), `FlaskRight/Wrong { topic }`,
  `BattleStart { monster }`, `BattleWon { monster }`, `PlayerHurt`, `Streak { n }`
  (5, 10), `LevelUp { level }`, `LowHealth`, and `Idle` (no event for about 25 s).
  Each cue carries a little run context: distance, level, streak, hearts, the last
  monster's name, the last topic. It never carries anything about the person
  playing.
- **The prompt** lives in a new `src/llm/villain_gen.rs` (plain Rust, no
  macroquad/egui, like `task_gen.rs`): the character sheet above, the rules
  (original character, silly and never mean, **mocks themself and their inventions,
  never the player**, no real people or brands, one short line, JSON only), and the
  cue. The reply goes through `extract_json_object` and serde into `VillainBeat`.
- **Asynchronous, the same way questions are** (invariant 1).
  `CerebrasClient::request_villain_async(cue) -> Receiver<Result<VillainBeat>>`
  reuses `request_raw_blocking` on a thread. `Game::update` polls it with `try_recv`
  next to `trigger_state.poll`, and still polls while paused, so a reply isn't lost.
- **Live questions always win.** Villain lines take the third of FR32's lanes, after
  live questions and question prefetches. No villain request starts while
  `trigger_state` is `Loading` or a question prefetch is due. At most one villain request is in flight. Cues that arrive while
  one is busy coalesce (the newest, highest-priority cue wins). There's a minimum
  gap of about 8 s between bubbles and a cap of `JEKO_VILLAIN_RPM` (default 6).
  `JEKO_VILLAIN=off` turns the live lines off; the doctor still appears, using the
  fallback bank.
- **Instant reactions through prefetch.** When a question opens, request *two*
  beats in the background, one for "right" and one for "wrong". When the answer
  lands, the matching one is already there, so the doctor reacts on the same frame
  and the other is dropped. That costs one extra small call per question, so it's
  behind a setting or the RPM cap (see open questions). Without prefetch, a reaction
  arrives about 0.3–1 s late, which is still fine for a bubble.
- **Offline fallback** (invariant 2's spirit). `assets/villain_lines.json`, compiled
  in with `include_str!` like the other assets, has a few lines per cue with a mood
  and action. It's used with no key, on a timeout or a bad payload, when rate
  limited, and with `JEKO_VILLAIN=off`. The doctor always says *something*.
  `gen_content` could gain `--villain` to regenerate this bank offline.
- **Token panel.** Villain calls are recorded in `TokenStats` like any other.
  Tag each `RequestUsage` with a kind (`question | villain`, extending FR21's
  `question | monster | lesson` idea) so the panel can split throughput.

**Cosmetic by design.** In this FR the villain changes nothing a rule reads (invariant
7). They never call `damage_player` or `request_task`, and they don't touch XP,
speed, answers or battle outcomes. Their reactions follow what the game already
decided.

### Milestones (each lands on its own)

1. **A: the doctor on the road, no LLM.** `villain.rs` drawing and all behaviours,
   the top-of-road movement and `Scurry`, `Tall::Villain` in the sort, the gate
   `Appear`, the speech bubble, cues from game events, and `assets/villain_lines.json`
   for every cue. Fully playable offline.
2. **B: live beats from Cerebras.** `villain_gen.rs` prompt and validation,
   `request_villain_async`, polling, coalescing and rate limiting, live-questions-win,
   prefetching right/wrong reactions, and token tagging.
3. **C: schemes** (best after FR21/FR22). Every ~300 m the doctor announces a scheme,
   such as `{"name": "Operation Fraction Fog", "topic": "fractions", "gadget":
   "fog_machine"}`. `gadget` is a closed enum of **cosmetic** effects (fog tint,
   bubble rain, a sign on the cart, the flask's colour). `topic` is passed to
   `Game::request_task` as a topic hint for the next few flasks and battles, so it
   still goes through the one door (invariant 6) and the difficulty still comes from
   the level. With FR21's `Studio`, the doctor is shown `Tinker`ing or `Brew`ing while
   a `GenMonster` job is in flight and holds up the flask when a new species lands:
   a live, in-world view of the background generation.

### Open questions

- **Name, look and lore.** Is "Dr. Nix Nullwell, who wants every number to be zero"
  right, or does the owner want a different name, look or motive? Are they
  connected to Professor Pebble (an old lab partner, say)?
- **Tone on a wrong answer.** The proposal says the doctor gloats about their own
  plan but never mocks the player, since this is a learning game for kids. Should a
  wrong answer get a gloat at all, or only a neutral "Hmm, interesting…"?
- **The "ultimate" fight.** Is there ever a showdown? On an endless road, a choice
  is a boss battle every 1000 m (after which the doctor flees in their cart and
  shows up again further on), or a single finale at a set distance. That needs
  battle work, so it would be its own FR built on FR2/FR3. *(Raised by FR27: the
  owner wants the road to end at a village with the mad scientist's castle, which
  points to a single finale there.)*
- **Moderation.** Are a schema, a length cap, a blocklist and a kid-safe prompt
  enough for live lines, or should live lines be optional, with the reviewed
  fallback bank as the default?
- **Cost.** Is the prefetch of two reactions per question worth the extra calls?
  What RPM and session token budget are acceptable?
- **Visibility.** Does the bubble hide with `show_hud` off, or get its own "Villain
  chatter" toggle in settings? Does the doctor appear in the background of the
  battle view (a cameo cheering on their monster)?
- **Schemes and questions.** Should a scheme actually steer question topics
  (Milestone C), or stay pure flavour? Steering changes what the player is asked,
  which touches FR3/FR22's question about subjects.

### Acceptance criteria

- [ ] Past the gate, a mad scientist with frizzy hair, goggles, a lab coat and a
  bubbling flask is visible near the top of the road and keeps moving around
  (pacing, tinkering, brewing, cackling) as the player walks.
- [ ] They stay ahead as the player progresses, sprinting included, and are never
  reached. They draw in the right depth order with scenery, freeze with pause and
  are hidden in the hub, battles and dialogue.
- [ ] They react to game events (flask right or wrong, battle won, streak, level-up,
  milestones) with a speech bubble and a matching animation.
- [ ] With an API key, the lines are generated live by Cerebras from the run's
  context. The frame never blocks (invariant 1), and no villain request delays or
  overlaps a live question.
- [ ] With no key, no network, a bad payload, rate limiting or `JEKO_VILLAIN=off`,
  lines come from `assets/villain_lines.json` and the game plays as before.
- [ ] Beat validation is a pure, unit-tested function. Hostile input (unknown enums,
  over-long lines, blocklisted words, missing fields) is clamped, defaulted or
  replaced.
- [ ] The villain changes no XP, speed, answer, HP or battle outcome (invariant 7),
  and every glow, spark and puff honours reduce flashing (invariant 9).
- [ ] The token panel shows villain calls separately from question calls.
- [ ] New dev events let it be checked without the window: e.g. `villain:<cue>`
  (fire a cue), `tovillain` (snap the camera so the doctor is on screen), plus the
  `JEKO_VILLAIN` / `JEKO_VILLAIN_RPM` env vars.
- [ ] `docs/ARCHITECTURE.md` (new `game/villain.rs` and `llm/villain_gen.rs`, the
  villain poll in the frame, `Tall::Villain` in the draw order, the new asset, env
  vars, dev events and tests) and the README layout are updated.

---

# Art direction — setting

**Goal:** make the game look like one place. FR24 turned the world into a dark
forest at night with the mad scientist's manor across a black lake. The characters
still dress for the beach: the hero (`Player::draw`, "chibi beachgoer: straw sun hat,
Hawaiian shirt, swim shorts") and the monster roster in `src/game/monster.rs`
(Pinchling the hermit crab, Jellyglow, Puffspike the pufferfish, Coco Knight,
Gull Bandit, Dune Wyrm, Tide Titan).

## FR25. Restyle the hero and the monsters for the dark forest

**Status.** **Not started.** Split out of FR24, which deliberately left characters
alone.

**Depends on:** [] _(pairs with FR23: the monsters are meant to be the mad
scientist's creations; FR21's generated monsters would follow whatever theme lands
here)_

**Motivation.** A hero in swim shorts and a crab, a jellyfish and a coconut in a
moonlit forest break the mood FR24 built. Restyling them as the doctor's creations
ties the roster to the villain and the manor.

**Proposed approach.** *(suggested, not decided)* Keep every `MonsterKind`, its
stats (`MonsterInfo`), its behaviour and its animations; change only names and
drawing, so no rule or test changes. For example: Pinchling → a toadstool sprite,
Jellyglow → a lantern ghost (still stuns), Puffspike → a puffball mushroom (still
grows), Coco Knight → an armoured pumpkin, Gull Bandit → a crow bandit (still
steals the hat), Dune Wyrm → a root wyrm, Tide Titan → a stitched-together golem from
the lab. The hero gets a hooded cloak or explorer's coat with a lantern, and the hat
the Gull Bandit steals becomes whatever hat the new outfit has. The FR16 roster line,
the dev event names (`pinch`, `gull`, …) and the architecture doc's monster table
follow the renames.

**Open questions.**

- Restyle in place (same kinds, new looks and names), or design a new roster?
- What does the hero look like now, and does the stolen-hat gag stay?
- Do the monsters show they were made in the lab (stitches, bolts, green glow)?

**Acceptance criteria.**

- [ ] The hero and every monster fit the forest; no beach creature or outfit is left.
- [ ] Every monster keeps its stats and special (stun, growing, hat theft, boss), and
  the existing tests pass unchanged.
- [ ] Monster names in the battle view, toasts, FR16 and `docs/ARCHITECTURE.md` match.

---

# Tooling — CI/CD

**Goal:** every push and pull request on GitHub is checked automatically, by the
same pipeline a developer can run locally. Today nothing runs in CI: the checks
are `make fmt`, `make lint` and `make test`, run by hand in the nix dev shell, and
there is no `.github/` directory.

## FR19. Dagger CI/CD pipeline, run from GitHub Actions

**Status.** **Not started.** Owner chose the Rust SDK, the flake as the build
environment, and Linux-only CD (2026-11-21). Still open: the release artifact,
`-D warnings`, branch protection.

**Depends on:** []

**Motivation.** Nothing stops a change that breaks the build, fails a unit test or
trips clippy from landing on `main`. Writing the pipeline in [Dagger](https://dagger.io)
rather than straight GitHub Actions YAML means it runs the same in a container on
a laptop (`dagger run …`) as on GitHub, so a red CI run can be reproduced locally
instead of debugged by pushing commits. GitHub Actions becomes a thin trigger.

**Decided (owner, 2026-11-21).** The pipeline is written with Dagger's **Rust SDK**,
the build container runs every step through **`flake.nix`**, and CD targets
**Linux only**.

**Proposed approach.** *(the three decisions above are fixed; the rest is suggested)*

- **The pipeline crate.** A small Rust crate in `ci/` (its own `Cargo.toml`, not a
  dependency of the game, so `dagger-sdk` never enters the game build) with one
  subcommand per step, each running against the repo's source directory:
  - `fmt` — `cargo fmt --check`
  - `lint` — `cargo clippy --all-targets -- -D warnings`
  - `test` — `cargo test`
  - `build` — `cargo build --release`, exporting the `jeko` binary
  - `check` — runs `fmt`, `lint`, `test` and `build`, so one call is the whole of CI.

  The Rust SDK (the `dagger-sdk` crate) is a client library rather than one of
  Dagger's module SDKs, so the entry point is `dagger run cargo run --manifest-path
  ci/Cargo.toml -- check`, not `dagger call`. Check whether that has changed
  before building.
- **The build container.** A `nixos/nix` image with flakes enabled
  (`--extra-experimental-features 'nix-command flakes'`) runs each step as
  `nix develop --command cargo …`, so CI uses the toolchain pinned in `flake.lock`
  and the libraries in `runtimeLibsFor` (libGL, X11, xkbcommon, ALSA). The library
  list then lives only in the flake. Mount cache volumes for `~/.cargo/registry`
  and `target/`, and find a way to keep the nix store between runs, because
  fetching the dev shell is the slow part.
- **No API key in CI.** The unit tests run offline, and the game falls back to
  `assets/tasks_fallback.json` without a key, so CI never needs `CEREBRAS_API_KEY`.
  Don't pass it, and keep `.env` and `target/` out of the source directory the
  pipeline uploads.
- **GitHub Actions.** `.github/workflows/ci.yml` runs on `push` to `main` and on
  every `pull_request` on `ubuntu-latest`. It checks out the repo, installs Rust
  and a pinned Dagger CLI (e.g. with `dagger/dagger-for-github`), and runs the
  `check` entry point above.
- **CD (Linux only).** On a version tag (`v*`), a second job runs `build` and
  attaches the x86_64 Linux binary to a GitHub Release. No macOS or Windows builds.
  A binary built inside the nix dev shell links against `/nix/store` paths
  (glibc's loader, libGL), so it only runs where those paths exist. Settle what the
  release artifact is before building CD (see open questions).
- **Local entry points.** Add `make ci` (the same `dagger run …` command) to the
  `Makefile` and the `dagger` CLI to the dev shell in `flake.nix`, and document both
  in the README's setup section.

**Open questions.**

- The release artifact: a binary for nix users only, one patched with `patchelf`
  to use the system loader and libraries so it runs on ordinary distros (which then
  need libGL, X11 and ALSA installed), or the output of `nix build`?
- Should clippy warnings fail the build (`-D warnings`)? The code may need a lint
  pass first.
- Should CI be a required check on `main` (branch protection)? That's a GitHub
  repo setting, not code.

**Acceptance criteria.**

- [ ] A Rust Dagger pipeline in `ci/` runs fmt, clippy, tests and a release build
  through the flake's dev shell, and its `check` passes locally on a clean checkout.
- [ ] A GitHub Actions workflow runs that check on every push to `main` and every
  pull request, and a failing test, lint or format check turns the run red.
- [ ] CI runs without `CEREBRAS_API_KEY` and never reads `.env`.
- [ ] Repeat runs reuse the cargo cache rather than rebuilding every dependency.
- [ ] A `v*` tag publishes a GitHub Release with the agreed x86_64 Linux artifact
  (per the open question above).
- [ ] `make ci` and the `dagger` CLI in the dev shell let a developer run the same
  pipeline locally; README and *Content, configuration and dev hooks* in
  `docs/ARCHITECTURE.md` describe it.

---

# Tooling — agents

**Goal:** run the game's model-driven jobs (content, monsters, lessons, the
villain, playtesting) as OpenClaw agents the owner can schedule, steer and talk to,
without making the game depend on them.

## FR30. Run JEKO's agent-shaped jobs as OpenClaw agents

**Status.** **Not started.** Requested by the owner; the approach is suggested only.
Check the OpenClaw details below (endpoints, config keys, skill layout) against
[docs.openclaw.ai](https://docs.openclaw.ai) before building. They were read on
2026-11-21 and OpenClaw changes quickly.

**Depends on:** [] _(milestone A needs nothing; milestone B shares FR21's content dir
and watcher, and the agents it enables are FR21's monsters, FR22's lessons and FR23's
villain; FR19's CI is a natural place for the playtest agent)_

**Motivation.** Several jobs in JEKO already are agents, or are planned to be: work
that runs in the background, calls a model, and writes something for the game to
pick up. Today each one is wired by hand: `make gen-content` is run manually, and
FR21/FR22/FR23 each plan their own Cerebras loop inside the game. OpenClaw is a
self-hosted agent gateway with skills (`SKILL.md`), cron jobs, sub-agents, inbound
webhooks and chat channels (Telegram, WhatsApp, Discord, …). Running these jobs
there gives the owner one place to schedule them, change their instructions without
a rebuild, see what they did, and message them from a phone ("make three swamp
monsters for tomorrow").

**What the owner asked for.**

- Integrate everything that can be an agent as an OpenClaw agent.
- A plan for how to introduce OpenClaw to the project.

**What can be an agent, and what can't.**

| Job | Today / planned | As an OpenClaw agent? |
|---|---|---|
| Live questions at a flask or in a battle | `CerebrasClient::request_task_async`, `llm/task_gen.rs` | **No, stays direct.** It's on the play path: the player is frozen until it arrives, and the offline fallback has to be instant (invariants 1 and 2). A gateway hop adds latency and a moving part. An agent can *pre-generate* question packs instead (see lessons). Those packs land in FR32's `QuestionBuffer`, the same place the game's own prefetches go. |
| NPC dialogue (`assets/dialogue.json`) | `make gen-content`, run by hand | **Yes, first.** A skill that runs the existing generator, checks the output, and opens a PR or leaves a diff. A cron job can refresh it weekly. |
| Generated monsters | FR21's `Studio` writing `content/monsters/<id>.json` | **Yes.** A "monster designer" agent writes `MonsterSpec` JSON into the content dir; FR21's watcher and validation load it. The in-game `Studio` stays as the offline-friendly path. |
| Lesson packs, explanations, field guide | FR22 | **Yes.** A "lesson writer" agent fills `lessons/<topic>-<level>.json`, steered by the run log (topics the player gets wrong). |
| The mad scientist's lines and schemes | FR23 | **Partly.** A "villain" agent with a persistent persona and memory across runs writes a queue of lines and schemes ahead of time; the game plays from the queue and falls back to the bank when it's empty. Live per-event lines stay direct to Cerebras if they must land within a second. |
| Playtesting | `JEKO_SHOT`, `JEKO_DEV` scripted events | **Yes.** A "playtester" agent runs scripted sessions, reads the screenshots, and reports what looks broken. Pairs with FR19's CI. |
| Progress reports | nothing yet | **Yes, new.** A "tutor" agent reads the run log and sends a short summary to a parent or teacher over a chat channel ("40 problems today, fractions still tricky"). Needs the owner's decision on what child data leaves the machine. |
| Feature work on this repo | coding agents reading this file | **Out of scope here.** OpenClaw could drive a coding agent, but the handoff through this file already works; revisit separately if wanted. |

**Proposed approach.** *(suggested, not decided)*

- **The rule: the game never needs OpenClaw.** OpenClaw is an optional producer
  beside the game, never inside the frame. The game talks to it only through files
  and, later, a fire-and-forget event post on a background thread. Without OpenClaw
  the game plays exactly as today (invariants 1 and 2). This is the boundary that
  keeps the integration safe to add one agent at a time.
- **The file contract.** Two directories make the whole interface:
  - **In:** FR21's content dir (`JEKO_CONTENT_DIR`, `content/`). Agents write
    `monsters/`, `lessons/`, `villain/` files there, atomically (write a temp file,
    then rename). The game's watcher validates and loads them, and a bad file keeps
    the last good version. The game doesn't know or care who wrote a file.
  - **Out:** a run log, `content/runs/<date>.jsonl`, that the game appends to (flask
    solved or missed with the topic, battle won or lost, death, distance). Agents
    read it to adapt. Nothing personal goes in it beyond what the owner approves.
- **Skills live in the repo.** An `openclaw/` folder with one skill per agent
  (`openclaw/skills/jeko-dialogue/SKILL.md`, `jeko-monsters/`, `jeko-lessons/`,
  `jeko-villain/`, `jeko-playtest/`, `jeko-tutor/`). Each `SKILL.md` says what the
  agent writes, the schema it must match (pointing at the serde types and an example
  file), the kid-safe rules, and the command to validate its output. A small
  validator binary (`src/bin/validate_content.rs`, reusing FR21's pure validation)
  lets an agent check its own work before the game sees it.
- **Agent config, also in the repo.** `openclaw/README.md` describes the OpenClaw
  config: one agent per job (`agents.list[]`), each limited to its own skill and
  given write access only to its part of the content dir; cron entries (nightly
  dialogue refresh, a monster batch before play time); the model provider (Cerebras,
  if OpenClaw can reach it through an OpenAI-compatible endpoint, so prompts and
  costs stay where they are today).
- **Live events, later.** Once the file contract works, the game can optionally
  `POST` events to the Gateway's inbound hook (`/hooks/agent`, with the hook token
  and `hooks.allowedAgentIds` limiting which agents can be woken) from a background
  thread, so the villain agent reacts within seconds instead of on a schedule.
  Env vars `JEKO_AGENT_URL` and `JEKO_AGENT_TOKEN`. Unset means off. Replies still
  come back as files. OpenClaw has no outbound webhook to push results to an app.
  This bridge lives in `llm/` or a new non-graphics module (invariant 10).
- **Safety.** ClawHub skills from third parties are untrusted. Only the repo's own
  skills are installed for these agents. The Gateway listens on localhost only.
  Everything an agent writes goes through the same validation as FR21's specs
  (schema, length caps, blocklist, kid-safe prompt), because a child plays the game.

**How the owner introduces it** (milestones, each lands on its own):

1. **A. Try it with no game changes.** Install OpenClaw locally and start the
   Gateway. Add the repo's `openclaw/skills/jeko-dialogue/` as a workspace skill
   (symlink it into `~/.openclaw/workspace/skills/`, or point the agent's workspace
   at the repo). Ask the agent in chat to refresh the dialogue. It runs
   `make gen-content`, checks the JSON, and shows the diff. Add a weekly cron. If
   this feels useful, carry on; if not, nothing in the game changed.
2. **B. The file contract.** Build FR21 milestone A's content dir and watcher (if not
   already built), the run log, and the validator binary. Then add the monster and
   lesson agents, which only write files. The game picks them up live.
3. **C. Playtester.** A skill that runs `JEKO_DEV` and `JEKO_SHOT` scenarios and
   reports on the screenshots. Optionally run it from FR19's CI.
4. **D. Live bridge and the villain.** Add the optional event post and FR23's
   villain agent with a line queue.
5. **E. Tutor reports.** Connect a chat channel and the tutor agent, once the owner
   has decided what it may send.

**Open questions.**

- **Which jobs first?** The order above starts with the safest (dialogue), but the
  owner may want monsters or the villain sooner.
- **Where does OpenClaw run?** On the dev machine only, or on an always-on box so
  cron jobs fire when the laptop is shut? (Cron runs inside the Gateway, so it needs
  the Gateway running.)
- **Which model?** Cerebras for everything (one key, one bill), or a different model
  for agents that need more reasoning (the villain's schemes, the playtester reading
  screenshots)?
- **Do generated files get committed?** Agent output straight into the running game's
  content dir, or through a PR the owner reviews first? (FR21 has the same question
  for `content/`.)
- **Child data.** What may the run log record, and may any of it leave the machine
  through a chat channel? Who gets the tutor reports?
- **Does the villain need to be live?** Is a queue refreshed every few minutes good
  enough, or do lines have to react to the exact moment?
- **Coding agents.** Should OpenClaw also drive coding agents over this file, or keep
  that separate?

**Acceptance criteria.**

- [ ] With OpenClaw absent, the game builds, runs and plays exactly as before, with
  and without `CEREBRAS_API_KEY`.
- [ ] `openclaw/` holds a skill per agent and a README with the config and setup
  steps, and following it from a clean OpenClaw install gets the dialogue agent
  working.
- [ ] The dialogue agent refreshes `assets/dialogue.json` on request and on a
  schedule, and its output passes validation.
- [ ] Monster and lesson agents write files the game loads live, through the same
  validation as FR21's specs, and a bad file never crashes the game.
- [ ] The game writes a run log the agents can read, with only the fields the owner
  approved.
- [ ] If the live bridge is built, it's off unless `JEKO_AGENT_URL` is set, never
  blocks the frame, and failures are silent apart from a dev-mode toast.
- [ ] `docs/ARCHITECTURE.md` (*Content, configuration and dev hooks*: the agent dirs,
  run log, env vars, validator; *The question pipeline* if any question path changes)
  and the README layout are updated.

---

## Completed

When an entry is fully built, delete it above and add one line here, newest
first: `FR<n> — title — date — why it closed (commit)`. Its number stays retired.

- FR31 — A goalpost tracker at the top centre — 2026-11-21 — built in one pass:
  `World::next_goalpost` (with the pure `goalposts_in_chunk` and `pick_goalpost`, 7
  tests: deterministic, always north, never a solved flask or beaten monster, the
  bigger event ranks first, a blocker counts down to the line it holds) feeds
  `game/goalpost.rs` (`Tracker`, 7 tests for the pop, unroll, countdown, pulse,
  "Now!", close and hide), painted by `ui/goalpost_panel.rs`; the toast steps down
  below it. New dev events `toroad`, `neargoal`, `atgoal`. Checked by screenshot:
  the circle, the pill with its ring, "Now!". Provisional answers to its open
  questions are in FR16. FR27's regions, FR28's stops and FR29's obstacles each add a
  `GoalKind` when they land

- FR3 — A battle is an education Q&A — 2026-11-21 — nothing left to build once FR2
  landed: FR10's battles already ask every question through the flask pipeline
  (`request_task`, `submit_answer`) with the fallback bank, and FR2's blockers start
  the same battle, so winning clears the road. Checked end to end with a key set and
  an unreachable API (`toblocker`, `e`, `wrong`, `right`): fallback questions, a wrong
  answer cost the monster's hit, the right one won and opened the road. Its open
  questions (subjects, the cost of a wrong answer) are in FR16

- FR2 — Monsters blocking the road ahead — 2026-11-21 — built on FR10's encounter
  zones rather than as a new system: an `EncounterZone` with `blocks` set stands in
  the open in the middle of the road (every sixth chunk from chunk 3, ~80 m apart, and
  every Tide Titan), drawn in the depth sort with its own `MonsterCanvas` behind a
  green rune ward. `World::clamp_blocked` stops the player 44 px short of it until it's
  beaten, E within reach starts the usual battle, and winning opens the road. Tests
  for placement and determinism and for the clamp; new dev event `toblocker`;
  provisional answers to its open questions are in FR16

- FR1 — The doctor's curse: a sickness that sets in once the player is on the road —
  2026-11-21 — reworked from rising lava at the owner's request; owner chose the
  framing (curse felt as a fever), a health drain at full, a full pause during
  questions and battles, and a slow lift in the hub. Built as `game/curse.rs`
  (8 tests): dormant until the player first passes the gate, rises over time and is
  held back by new ground, cleansed by a correct flask, slows the player past the
  fever stage and drains ½ heart every 3 s at full through `damage_player`; a green
  tint and closing vignette, wisps off the hero, a curse meter under the hearts,
  stage toasts, dev events `curse`/`cursefull`. Remaining provisional numbers are in FR16

- FR26 — Skill bar with roll charges and sprint stamina — 2026-11-21 — requested
  directly by the owner and built in one pass: `game/skills.rs` (`RollCharges`: 3
  rolls, a 1.0 s trickle, a 1.5 s lockout; `Stamina`: 3 s of sprint, 4 s refill after
  a 0.5 s delay, winded until 35%; 5 unit tests) owned by `Player`, and `draw_bar`,
  keycap slots at the bottom centre with charge pips, a lockout sweep and countdown,
  a stamina meter and a "Winded" state. New dev event `spent`; controls reference,
  README and ARCHITECTURE updated. Provisional answers are in FR16 (uncommitted)

- FR24 — Revamp the map from the beach to a dark forest with a gothic manor —
  2026-11-21 — owner chose to replace the beach entirely and to use potion flasks as
  pickups; all four milestones built in one pass: night palette and gradient lights in
  `gfx.rs` (`glow`, `vignette`, `potion_flask`), lake, bank, forest floor and woods,
  a raised cobbled road with iron fences and lanterns, forest props and the hub camp
  in `decor.rs`, the manor backdrop with reflection, bats and lightning in the new
  `game/manor.rs` (tested), mist, fireflies, light pools, fern encounter patches,
  flasks on rune stones, a moonlit battle backdrop, a parchment-and-dark-wood egui
  theme, forest wording and renames (`ROAD_SOUTH_END`, `ROAD_LIFT`, `draw_road`,
  `bank_x`, `treeline_x`, `Tall::Flask`, `flasks_solved`, `toflask`), new dev events
  `nohud` and `calm`, and opaque `JEKO_SHOT` screenshots. Open FRs' wording updated;
  FR1's lava and FR17's shells got a theme question; provisional choices in FR16; the
  hero and monsters split out as FR25 (uncommitted)

- FR20 — A bow the hero can shoot — 2026-11-21 — requested directly and built in one
  pass: `game/bow.rs` (shot timing, slung/held bow, arrow drawing, cosmetic `Arrow`s
  that arc, stick or splash, tested), `Action::Shoot` on F with a raise/draw/release
  animation, arrows in the depth sort, and the battle attack shown as a bow shot
  (`battle::draw_shot`). Provisional answers (cosmetic arrows, key, no ammo) are in FR16
- FR18 — Sprint on Shift, roll moved to Ctrl — 2026-11-21 — built: holding Shift
  multiplies walk speed by `SPRINT_MULTIPLIER` (1.6, stacks with Speed Boost) in
  `Player::update`; roll rebound to Ctrl; controls reference updated (578161f)
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
