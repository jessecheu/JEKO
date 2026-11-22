---
type: Feature Request
title: JEKO — Feature Requests
description: Concrete, actionable feature requests for JEKO (Math Quest) — work that is scoped and wanted but not yet built. Each entry states the motivation, a proposed approach, and what "done" looks like. Maintained as the shared agent todo list.
tags: [feature-request, backlog, creation-bar, progress-circle, gameplay, urgency, curse, sickness, monsters, battle, education, minigames, pets, cat, controls, pause, settings, health, death, art-direction, spawn, tooling, ci-cd, dagger, content-generation, hot-reload, live-content, villain, story, live-dialogue, setting, forest, manor, map, exploration, regions, village, castle, rest-stops, road, obstacles, agents, openclaw, hud, goalposts, rate-limits, scheduler, streaming, stretch-packs, landmarks, npcs, narration, lighting, weather]
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

**The next free number is FR42**; bump it when you file one. Numbers are never
reused or renumbered when an entry is removed. Take the next number, not the first gap.

---

# Gameplay — urgency

**Goal:** make the player feel urgency so they keep moving up the road.
The doctor's curse (FR1, built) now wears the player down while they linger on the
road, and monsters standing on the road (FR2, built) shut it until they're beaten
in a question battle (FR3, built). Both are done; their open questions are in FR16.

# Gameplay — minigames & companions

**Goal:** break up the walk with small optional moments that reward the player
without changing the game's balance. FR34 is the first: a caged pet cat. It sets
the pattern (a thing on the road, a quick puzzle in a modal, a cosmetic reward)
that later minigames can copy.

## FR34. Unlock a pet cat from a cage

**Status.** **Not started.** Reworked from a pet lion to a pet cat at the owner's
request (2026-11-21), and renumbered from FR4, a number the token usage panel
already had (see Completed), then from FR32 to FR33, then to FR34 because the
prefetch FR (*Live content*) held FR32 and the creation bar took FR33; the approach is suggested only.

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

## FR16. Confirm the provisional answers from FR1–FR3, FR5–FR15, FR17, FR20–FR24, FR26, FR27, FR28, FR31, FR32 and FR33

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
  *Built as FR17: gears (see the FR17 item below).*
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

- [ ] **FR21** Generated monsters on the road. Invariant 3: *the owner chose to
  freeze the roster at the start of a run, so zone placement never depends on the
  content dir and a studio monster joins on the next run.* Provisional: where does
  generated content live? *The content dir, a per-machine cache that persists between
  runs; `gen-*.json` is gitignored, and renaming one keeps it for committing.* Which
  specials may generated monsters use? *`none` and `grows` only.* Moderation? *A
  kid-safe, original-creature prompt, a whole-word blocklist and the schema; no human
  approval step.* Which model? *`CEREBRAS_MODEL`, no `JEKO_STUDIO_MODEL`.* How many
  zones get a spec monster? *40% (`SPEC_SHARE`), from every spec in the pool,
  hand-written ones too, never a boss.* Limits? *6 requests a minute, 60,000 tokens a
  session, one job at a time, three unseen monsters kept in stock for the tier ahead.*
  Do the seven built-ins move into specs? *No, they stay hand-coded.*

- [ ] **FR22** Education content. Math only, or other subjects? *Math only, numeric
  answers (a new answer type would be needed otherwise; FR3's open question).* Show the
  explanation on every wrong answer, or only after two misses? *On every wrong answer, when
  the question has steps (the offline bank's do).* Are generated field-guide facts shown as
  generated? *Spec monsters' facts are shown as their file says (hand-written ones are
  reviewed by whoever commits them; a studio monster's fact is vetted by the blocklist
  only); the seven built-ins' are hand-written.* Does per-topic accuracy persist between
  runs? *No: it lives in memory for the run. The field guide lasts for the session and isn't
  saved either. Persisting learner data is the owner's privacy decision, with FR14.* Other
  provisional numbers: a topic is weak below 75% after at least 2 tries; a pack is 8
  problems (4 for similar practice) and is asked for when fewer than 3 questions are ready
  at the player's level; the last 4 misses are kept; lesson jobs and tips share the studio's
  6 requests a minute and 60,000 tokens.

- [ ] **FR32** Asking ahead. Level-ups? *Take the exact difficulty, else one below, else
  ask live.* Does the buffer survive a new run? *Yes.* Wasted calls? *About 1.5× the
  calls is accepted; 20 look-ahead requests a minute at most.* The soft deadline?
  *1.5 s in a battle and 2.5 s at a flask; a slow model gets a bank question, and the
  late answer is kept.* An invariant ("the player never waits on the network when a
  buffered or fallback question could be shown")? *Not added; §5 of the architecture
  doc states the rule.*

- [ ] **FR33** Real progress or a stage ring? *A stage ring: an arc turns while it
  writes and closes when it lands; no streaming, so no percentage.* Share the top centre
  with the goalpost tracker? *The bar sits under the tracker and pushes the toast down.*
  How loud? *Shown whenever something is being generated or has just been (2.2 s), hidden
  in the hub, dialogue and map, and in a battle only the battle's question; a setting turns
  it off. It stays up while a flask problem is open, so the ready pop shows.* Do failures
  show? *A fallback question shows quietly as a grey "bank" circle; a failed look-ahead or a
  turned-down studio job just disappears.* Does the player see what the model wrote?
  *Only in the hover, and only once it is made: a monster's name, a pack's topic.*
  Stock line under the circles? *Yes: look-ahead questions and unseen monsters, as counts.*

- [ ] **FR27** The castle. How far? *1,500 m, and the road ends there (the owner's pick).* What
  happens on arrival? *A win screen with the run's stats and "Walk the road again", "to be
  continued" (the owner's pick); the gate rises while it comes up; no showdown.* The manor and the
  castle? *One building: the castle is the manor up close, and the backdrop manor fades out as it
  nears (the owner's pick).* The regions? *A const table in `game/regions.rs` (the owner's pick), names
  hidden in the fog; provisional borders at 900 m (Tallybrook Fields), 1,100 m (Tallybrook Village) and
  1,400 m (The Castle Grounds), names and colours, and what each region looks like.* What is the
  castle's plaque and name? *"THE CASTLE"; the doctor isn't named on it.* Does the doctor go in
  or stay on the road? *They duck inside once you're within 16 m of the gate, so they're still never
  caught.* Does the map persist between runs? *No: it resets with the run, like all state.* How
  detailed is the map? *Landmarks plus flasks, as milestone A had it; the minimap shows the castle as a
  purple square.* Is there a boss at the gate? *No: no Titan at 1,500 m; the Titans are at 500 and 1,000 m.*
  A safe stop before the castle? *Yes, at 1,460 m.*
- [ ] **FR17** What is the coin in the forest? *Gears: brass cogs dropped from the mad
  scientist's lab (the owner's pick).* What are they for? *Spending at a safe stop's shop
  (the owner's pick): hot soup (4 gears, 2 hearts back) and a heart charm (12 gears, one
  more heart, up to 8), provisional prices.* Do they reset on death? *Yes (the owner's
  pick): `RunStats.gears`.* How many? *Provisional: a trail of 3 to 5 in about a third of
  chunks, 1 gear each, so roughly 20 gears between stops; no gear is worth more.* Do they
  show in the HUD? *Yes, in the progress panel (so hidden with it), and the Game Over panel lists
  the unspent ones.*
- [ ] **FR28** Is a "quick mouse" a house? *A small house / rest stop with no mouse and no
  keeper (the owner's pick).* What does a stop do? *The curse lifts like the hub's
  (0.05/s), hearts mend 4× faster (½ heart every 2 s), and nothing can hurt the player
  (the owner's pick).* Checkpoint? *No: dying still restarts at the hub (the owner's
  pick).* How often? *Provisional: every 250 m, the first at 210 m, each 40 m before a
  round number so the ones before a Titan leave room to heal.* Can the player linger
  forever? *Yes, as in the hub: the curse lifts there and nothing hostile comes.* A keeper?
  *No: an empty cottage whose door is the shop.* Does it change invariant 4? *Yes, extended
  from the hub to a stop's span (recorded in the architecture doc).*

- [ ] **FR23** The mad scientist. **The owner's answers (2026-11-21):** live lines are on by
  default with a key (`JEKO_VILLAIN=off` keeps the bank); both reactions to a flask are asked for
  when it opens, under the cap; a scheme steers the next 3 flasks (not battles); the bubble has its
  own "Villain chatter" switch rather than following the HUD. **Still provisional:** the name, look
  and lore? *Dr. Nix Nullwell, who wants every number to be zero, as FR23 proposed; no link to
  Professor Pebble.* A wrong answer? *A gloat about the doctor's own plan, never at the player (the
  prompt says so).* Moderation? *A schema, a 90-character cap cut at a word, the word blocklist and a
  link filter; anything else turned down goes to the reviewed bank; no human review.* Cost? *6 live
  calls a minute (`JEKO_VILLAIN_RPM`), one out at a time, a flask's two reactions in one call; no
  session token budget.* How long does a due line wait for a live one? *2 s of game time, then the
  bank.* Schemes? *Every 300 m with none running, lasting 3 flasks or 250 m, whichever comes first; a
  toast says the topic; the topic must be one of `topics_for_level`; one fallback scheme name and
  gadget per topic.* A cameo in battles? *No; the doctor is hidden and no villain call starts in a
  battle.* The studio link? *Tinkering and brewing while a monster is built, and an `Unveil` with
  its name when it lands.* The showdown? *Filed as FR35.*

**Acceptance criteria.**

- [ ] The owner has confirmed or replaced every decision above, and any change
  that needs code is filed as its own FR.

---

# Live content — generated monsters & lessons

**Goal:** use Cerebras' fast inference (Kimi K3.1, `CEREBRAS_MODEL`) for more than
one question at a time. While the player walks, a background worker writes **new
monsters** and **education content** ahead of need, and the running game picks them
up through **hot reload**. FR21 (monsters as data, the studio, spec monsters on the road)
and FR22 (checked lesson packs, explanations, themed battles, adaptive practice, the
field guide, Pebble's tips) are built, and are in *Completed* below; the design as built is in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) (§5 and §8). What's left here is FR32's
last check and one optional refactor. The model's replies have never been seen against
the real model in the sandbox these were built in (no network), so the first thing to do with
a key is `make gen-monsters N=4`, play a few flasks and look at what the studio wrote in
`content/lessons/` and `content/monsters/`.

## FR32. No waiting at an encounter: get questions and monsters ready before the player arrives

**Status.** **Built (2026-11-21), apart from one check and one optional refactor.**
Done: `llm/buffer.rs`
(`QuestionBuffer`, the pure `plan_prefetch`, `WaitStats`), `game/prefetch.rs`
(`Prefetch`, `demand`, the limits and `JEKO_PREFETCH` / `JEKO_PREFETCH_RPM`),
`world::upcoming_in` and `World::upcoming`, `Game::request_task` taking from the buffer
first, the battle's first question asked in `start_battle` and each later one in
`battle_answer` (the "next" slot, shown when the phase reaches `Question`), the soft
deadlines (1.5 s battle, 2.5 s flask) that keep the late answer, `RequestKind` tags on
every call, the token panel's readout, and the `JEKO_DEV=fakemodel:<ms>` hook. The
priority order is live question > look-ahead > studio: look-ahead starts nothing while a
live question loads, and the studio (FR21, built) holds its job while either is in
flight (`live_busy`). A zone's species is bound from the run's frozen roster
(`World::pool`), so no encounter waits on a generation job. Checked by screenshot with
`fakemodel`: a flask opens `Active` from the buffer on the frame E is pressed, the panel
shows "ready on arrival 1/1 · longest wait 0.0 s" and the per-kind counts, and a 3 s
model in a battle trips the deadline (bank question after 1.5 s, the late answer lands
in the buffer).

**Left:** (1) check that a battle's later questions arrive ready in a real-time run
with `fakemodel:700`: scripted runs step game time at 1/60 s per frame while frames run
faster, so the 0.8 s attack covers less than 0.8 s of wall-clock and the stand-in model
looks slow. (2) Optional: live and look-ahead questions still spawn a thread each; one
shared worker with a priority queue would replace the `live_busy` flag. Nothing needs it
yet, and villain lines (FR23) would be its first user. *(Now filed as FR36, with a
shared rate budget.)*

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
- [x] Once FR21 milestone C lands, a zone's species is bound off screen from the
  loaded roster only, and no encounter waits on a generation job. *(Bound from the
  frozen pool; no job is ever waited on.)*
- [ ] `docs/ARCHITECTURE.md` is updated: §5 (the buffer before the live call, the
  deadline), §4 (where `update_battle` and the road frame request), §11 (the env
  vars and the `fakemodel` hook), §13 (the one-thread-per-request limit), and
  §12 if the owner adds an invariant. *(Done for A, B and the deadline; the lanes
  and the monster stock remain.)*

---

# Live content — spending the token budget

**Goal:** keep Cerebras busy writing things the player sees, all along the road, and
let the player *watch* it write. Today the game makes many small calls (a question, a
doctor's line or a tip is about 100–300 output tokens, written in a fraction of a
second), so it runs into the per-minute *request* cap long before the *token* cap:
the lane caps add up to more than 32 requests a minute (prefetch 20, villain 6, studio
6, plus live questions), and on the trial tier most calls come back 429 and fall to the
bank (*Known limits* in `docs/ARCHITECTURE.md`). Nothing streams, so a reply appears
all at once and the token panel only knows the speed after the call is over. The way to
use the budget is fewer, bigger calls under one shared rate budget (FR36), streamed so
the writing shows (FR37), filling the road with content a stretch at a time (FR38), with
scenery (FR39) and voices (FR40) built on the same machinery.

## FR36. One scheduler and one rate budget for every model call

**Status.** **Not started.** Filed 2026-11-21 at the owner's request; the approach is
suggested only. Takes over FR32's optional refactor (one shared worker) and the fix
named under *Known limits* ("one long-lived worker with a priority queue").

**Depends on:** [] _(the base for FR37 and FR38; replaces the `live_busy` flag between
the question lanes, the villain's `Voice` and the studio)_

**Motivation.** Every lane keeps its own per-minute cap (`JEKO_PREFETCH_RPM`,
`JEKO_VILLAIN_RPM`, `JEKO_STUDIO_RPM`) and spawns its own thread, and nothing knows what
Cerebras will actually allow. The caps add up to more than the trial tier takes, so calls
fail with 429, while the tokens per minute go mostly unused. One scheduler that knows the
real limits can send exactly as much as fits, most important first, and stop before a 429
instead of after.

**Proposed approach.** *(suggested, not decided)*

- **One worker, one queue.** A long-lived thread in `llm` (no macroquad, invariant 10)
  takes `Job`s with a priority: live question > battle's next question > look-ahead >
  villain line > stretch pack (FR38) > studio. Each job still answers over its own
  `mpsc` channel, so `TriggerState`, `Prefetch`, `Voice` and the studio poll exactly as
  today (invariant 1). A few calls may be out at once (a small pool, say 3), and a live
  question never waits behind a slow pack: it takes the next free slot, and if every slot
  is busy the soft deadline and the bank cover it as today (invariant 2).
- **A budget from the real limits.** Read Cerebras' `x-ratelimit-*` response headers
  (requests and tokens remaining, and when they reset; check the exact names against a
  real reply) into a `RateBudget` beside `TokenStats`. Before a call, estimate its tokens
  (prompt length plus `max_tokens`) and send it only if the budget has room; low-priority
  jobs leave headroom for live questions (say they may use only 70% of what's left). With
  no headers yet, start from conservative defaults (env: `JEKO_RPM`, `JEKO_TPM`).
- **429s back off.** A 429 marks the budget empty until its reset time and puts the job
  back in the queue (live questions go straight to the bank instead, as today).
- **Bigger calls.** Set `max_tokens` per kind and use JSON mode (`response_format`) where
  the model supports it, so large structured replies parse first time.
- **The old caps become shares.** The per-lane env vars stay as upper bounds per lane, so a
  player can still turn a lane down or off (`JEKO_VILLAIN=off` and friends keep working).
- **The token panel shows the budget:** requests and tokens left this minute, the queue by
  kind, and how much of the minute's token allowance was used.

**Open questions.**

- Which Cerebras plan is this for, and what are its request, token-per-minute and
  token-per-day limits? Should a session have a hard token cap of its own (like
  `JEKO_STUDIO_TOKENS`, but for everything)?
- How much headroom should background work leave for live questions?

**Acceptance criteria.**

- [ ] Every model call in the game (questions, look-ahead, villain, studio, packs) goes
  through the one scheduler; no lane spawns its own thread per call.
- [ ] Under the trial tier's limits, a 20-minute run with a key gets (close to) no 429s,
  and the token panel shows the share of the minute's tokens used.
- [ ] A live question is never delayed by background work beyond today's soft deadline,
  and with no key or network the game plays as it does today.
- [ ] The priority order, the budget and the back-off are unit-tested without a network
  (a fake clock and fake headers).
- [ ] `docs/ARCHITECTURE.md` (§1, §2, §5, §11 env vars, §13) and the README are updated.

## FR37. Stream replies and show the writing

**Status.** **Not started.** Filed 2026-11-21 at the owner's request; the approach is
suggested only.

**Depends on:** [36] _(streaming is a property of the shared worker; FR38–FR40 are what
it shows)_

**Motivation.** Cerebras writes at hundreds to thousands of tokens a second, and that
speed is the point of the project, but the player never sees it: `post_chat` waits for
the whole reply, and the dialogue box's typewriter (`Dialogue`) reveals text at a fixed
speed. Streaming lets long text appear at the model's own rate, and lets the token panel
and the creation bar show the speed *during* the call.

**Proposed approach.** *(suggested, not decided)*

- **Stream on the worker.** Send `stream: true` (and ask for usage in the last chunk if
  the API offers it, so `RequestUsage` still has real token counts) and read the
  server-sent events on the worker thread. Partial text goes down the job's channel as
  `Partial(String)` events, then `Done(reply)` or `Failed`.
- **Checked before shown, still.** Text a child reads must pass `content::vet` first, so
  only *prose* streams to the screen, a sentence (or clause) at a time: each finished
  sentence is vetted and cut to length on the worker, then sent. A sentence that fails
  ends the stream there and the rest comes from the bank. Questions do **not** stream
  visibly: their answer is verified against `expression` (FR22) before the problem is
  shown, as today.
- **Pulling prose out of JSON.** Replies are JSON, so add a small incremental reader that
  yields the growing value of named string fields (`"line"`, `"text"`, `"steps"`) while
  the rest of the object is still arriving.
- **On screen.** The dialogue box, signposts and notes (FR38), diary pages and battle
  narration (FR40) take streamed text instead of a fixed typewriter. The token panel
  shows live tokens per second while a stream is open, and a creation-bar circle can show
  the characters written so far.

**Open questions.**

- Is showing text a sentence at a time (after vetting) fast enough to feel live, or should
  shorter clauses be vetted and shown?
- Should the player be able to tap to skip a stream, as they can skip the typewriter?

**Acceptance criteria.**

- [ ] With a key, a long piece of prose (a Pebble line, a signpost) visibly writes itself
  at the model's speed, and the token panel shows the live rate while it does.
- [ ] Nothing reaches the screen before `content::vet` has passed it, and a blocked
  sentence ends the stream and falls back to the bank (unit-tested with a scripted stream).
- [ ] The incremental JSON reader is pure and unit-tested on replies cut at every byte.
- [ ] `fakemodel` streams too, so the whole path can be seen offline.
- [ ] `docs/ARCHITECTURE.md` (§1, §5, §7 if a panel changes, §9, §11) is updated.

## FR38. Stretch packs: the road written ahead, 100 m at a time

**Status.** **Not started.** Filed 2026-11-21 at the owner's request; the approach is
suggested only.

**Depends on:** [36] _(FR37 makes the text appear live; without it a pack's text just
appears whole. Reuses FR32's look-ahead and FR22's checks)_

**Motivation.** Most of the road is trees, fence posts and flasks, and the only words on it
are the questions and the doctor's bubble. One call that writes everything for the next
stretch of road (about 100 m, 7–8 chunks of 320 px) fills the map with things to read and
spends the token budget in a few large calls instead of many small ones.

**Proposed approach.** *(suggested, not decided)*

- **Slots, placed by the world.** Invariant 3 holds: *where* things are stays a pure
  function of the chunk index. The world places empty slots — a signpost, a note pinned to
  a tree, a carved stone, a traveller resting by the road — with a `slot_for_chunk` rule
  beside `spot_for_chunk`, kept clear of flasks, zones, gears and stops, with a determinism
  test. The model only fills in what a slot *says*.
- **One pack per stretch.** As the player nears a stretch (look-ahead, as FR32 does for
  questions), the scheduler asks for a `StretchPack`: the region and what has happened this
  run go in; out comes JSON with the text for each slot, 6–8 region-flavoured questions
  (each verified against its `expression`, FR22, then added to the question buffer), and
  flavour lines for the monsters in that stretch. A pack is 3–5k tokens.
- **A small quest per stretch** (if the owner allows it, see below): a traveller asks for
  help ("the miller needs ¾ of 24 sacks carried up to the mill"), and the stretch's next
  three flasks ask questions in that story, the way a scheme steers topics (invariant 7).
  Finishing it gets a cosmetic thank-you and a line in the map.
- **Offline and on failure,** each slot kind has a reviewed bank (`assets/stretch_bank.json`,
  checked like `villain_lines.json` with a `check_bank` and `make validate`), so the road
  reads the same with no key.
- **Checked.** Text through `content::vet` and length caps, questions through `llm::expr::verify`;
  a pack that fails its checks is dropped whole, never half-shown.

**Open questions.**

- May a stretch's quest steer the *text* of its flask questions (a story around the same maths)
  the way a scheme steers their topic? It changes no difficulty, judging, XP or HP.
- How long is a stretch: 100 m, a region, or between safe stops (250 m)?
- Should packs be saved to the content dir (like lesson packs) so a stretch written once is
  reused next run, or written fresh every run?

**Acceptance criteria.**

- [ ] Every stretch of road has readable slots (signposts, notes, travellers) whose text,
  with a key, comes from that stretch's pack, and from the bank without one.
- [ ] Slots are placed by a pure chunk rule, never on a flask, zone, gear or stop, with a
  determinism test.
- [ ] A pack's questions are verified before they enter the buffer, and its text is vetted.
- [ ] Over a run with a key, packs account for most of the session's tokens, and calls per
  minute stay under the plan's cap (the token panel shows it).
- [ ] `docs/ARCHITECTURE.md` (§5, §6, §11, §12 if an invariant changes) and the README are updated.

---

# World — the map and the destination

**Goal:** give the road somewhere to go and a way to see how far you've come.
The map on M (FR27, built) fills in as the player explores, the road runs through
four regions and ends at the castle gate 1,500 m up, where walking up to it wins the
run. Its open questions are in FR16. What's left of the story there (a showdown with
the mad scientist inside) is FR35.

---

# World — the road itself

**Goal:** make the road worth walking for its own sake. Today it's one cobbled lane
with an unbroken fence on both sides, a gentle sway, and nowhere to stop between
the hub and wherever the player dies. Places to rest along the way are built (FR28,
see Completed); still to come is a road that bends, narrows and is cluttered with
things to walk around (FR29).

## FR29. A road that winds, a fence that breaks, and things in the way

**Status.** **Not started.** Requested by the owner; the approach is suggested only.

**Depends on:** [] _(touches FR28's clearings (built) and FR27's map, which both read
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
  `PATH_HALF_W` constant. *(FR28 is built without this: a safe stop adds
  `stop_bulge(y) · STOP_EXTRA_W` to `walkable_x`'s right edge, and the fence and pad read
  `stop_bulge` too. Fold that into `path_half_w` when this lands.)*
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
  an encounter zone, a gear trail or a safe stop (FR28).
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
  a barrel to find a gear (FR17).
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

**Goal:** give the endless road a face and a reason. FR23 (built) put a mad
scientist just up the road who reacts to what the player does, with lines and
schemes Cerebras writes in the moment. What's left is an ending (FR35) and more live
voices along the road than the doctor's (FR40).

## FR35. A showdown with the doctor at the castle

**Status.** **Not started.** Split out of FR23 (built 2026-11-21), whose open question "the
'ultimate' fight" said a showdown would need battle work and be its own FR. The approach is
suggested only.

**Depends on:** [] _(builds on FR2/FR3's blocking battles, FR27's castle gate and win screen, and
FR23's doctor, live lines and schemes)_

**Motivation.** The doctor is always just up the road and ducks into the castle as the player
nears the gate, and the victory panel says "to be continued". A last battle with them would give the
run an ending that matches the chase.

**Suggested approach.** At the castle gate, before `Game::win`, the doctor comes back out for a
multi-phase battle (`Battle` with a doctor `Species`, or a new battle kind): one phase per scheme they
ran this run, each asking about that scheme's topic through `request_task`, with live taunts from
`villain_gen` between rounds and the bank as the fallback. Winning opens the gate as today.

**Open questions** (the owner's):

- Is there a showdown at all, or does the castle stay a quiet win?
- At the gate only, or a boss battle every 1,000 m as well?
- What does losing cost: a heart per wrong answer, as monsters do, or something gentler for the end?
- Does the doctor get a battle sprite of their own, or the road sprite scaled up?

**Acceptance criteria**

- [ ] The owner has answered the questions above.
- [ ] Reaching the gate starts the showdown, winning it shows the victory panel, and the game plays
  offline from the bank.
- [ ] `docs/ARCHITECTURE.md` and the README are updated.

---

## FR40. Live voices along the road: villagers, the doctor's diary, battle narration, explanations

**Status.** **Not started.** Filed 2026-11-21 at the owner's request; the approach is
suggested only. Can be built in parts, one voice at a time.

**Depends on:** [36, 37] _(streams through FR37; the diary and villagers can share FR38's
slots and packs)_

**Motivation.** The doctor (FR23) is the only character who speaks live. Tallybrook has houses
but nobody to talk to, Professor Pebble's lines are a fixed bank plus tips, battles are silent
between questions, and a wrong answer's explanation is the problem's canned steps. Each of these
is a place where live, streamed text makes the world feel written for this run.

**Proposed approach.** *(suggested, not decided)*

- **Villagers you can talk to.** A few villagers stand in Tallybrook Fields and Village (slots
  from a pure chunk rule, invariant 3). E opens the dialogue box; the villager's line streams in,
  written from their role and what has happened this run, and may end with a small word problem
  that goes through `request_task` and `submit_answer` like a flask (invariant 6). The bank
  (`assets/dialogue.json`, checked by `dialogue::check_bank`) is the offline voice.
- **The doctor's diary.** Pages dropped along the road (a slot kind from FR38) that the doctor
  "wrote" about this run: the schemes they ran, the monsters the player beat, a hint of what waits
  at the castle (sets up FR35). Read with E, streamed.
- **Battle narration.** One call at the start of a battle writes the monster's taunt and a line for
  each round (`hits` of them), streamed into the battle view between phases, never delaying a
  question. A bank line per monster offline.
- **Explanations for this mistake.** A wrong answer streams a short explanation of where *this*
  answer went wrong (the given answer and the expression go in), checked by the vet and length
  caps; the problem's own steps stay as the fallback.
- **Every line checked** before it's shown (FR37's sentence vetting), and every voice off with one
  setting, like `villain_chatter`.

**Open questions.**

- Which voices first? (Suggested: explanations, then villagers, then the diary, then battles.)
- May a villager's word problem reward anything (gears, XP), or is it cosmetic like the cat (FR34)?
- Should a personal explanation ever name the child's wrong answer, or only explain the right way?

**Acceptance criteria.**

- [ ] With a key, villagers, diary pages, battle narration and wrong-answer explanations stream
  live text that fits the run; with no key each falls back to a reviewed bank.
- [ ] No battle question and no flask question waits on narration or a villager's line.
- [ ] Every new bank has a `check_bank` run by a test and by `make validate`.
- [ ] `docs/ARCHITECTURE.md` (§5, §8, §9, §10 panels, §11) is updated.

---

# Art direction — setting

**Goal:** make the game look like one place. FR24 turned the world into a dark
forest at night with the mad scientist's manor across a black lake. The characters
still dress for the beach: the hero (`Player::draw`, "chibi beachgoer: straw sun hat,
Hawaiian shirt, swim shorts") and the monster roster in `src/game/monster.rs`
(Pinchling the hermit crab, Jellyglow, Puffspike the pufferfish, Coco Knight,
Gull Bandit, Dune Wyrm, Tide Titan). Restyling them is FR25. Beyond the characters,
FR39 lets the model design one-off landmarks for the road, and FR41 adds light, weather
and depth that change with the region.

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

## FR39. Landmark specs: scenery the model designs, drawn by a rig

**Status.** **Not started.** Filed 2026-11-21 at the owner's request; the approach is
suggested only.

**Depends on:** [] _(copies FR21's monster pipeline — studio, spec, vetting, watcher, hot
reload — for scenery; pairs with FR38, whose packs could name the landmark ahead, and with
FR25 and FR29)_

**Motivation.** The road's props are a fixed set (`decor::Kind`), so every stretch of a
region looks alike. Monsters already show how tokens become graphics: the model writes a
`MonsterSpec` in a closed vocabulary and `monster_rig::draw_spec` draws it. The same for
scenery would give each stretch a one-off landmark — a crooked shrine, a broken windmill,
a lab ruin leaking green light — and spend tokens on something the player sees.

**Proposed approach.** *(suggested, not decided)*

- **`LandmarkSpec`** in `content/` (plain Rust): a base shape (tower, hut, arch, ruin,
  statue, well, mill), parts with positions, sizes and colours from a closed set, roof and
  window styles, what lights it casts (feeding `Decoration::lights`), an optional idle
  motion (sails turning, smoke, flicker), a name and a one-line plaque. `parse` clamps and
  validates it, as `monster_spec::parse` does, including the palette check against `INK`.
- **`landmark_rig::draw_spec`** in `game/` draws it with the `gfx` vocabulary, depth-sorted
  as a `Tall` item.
- **The studio designs them** (`Job::Landmark`, lowest priority, through FR36's scheduler when
  that lands) into `<content dir>/landmarks/`, and the watcher loads them.
- **Placement stays pure.** A landmark slot every so often (say one per 100 m, beside the road)
  comes from the chunk index; which spec stands in it comes from the run's frozen pool, as spec
  monsters do (invariant 3). An empty pool shows a hand-written landmark.

**Open questions.**

- How often should a landmark appear, and only beside the road or sometimes across it (an arch)?
- Can a landmark be visited (its plaque read with E), or is it scenery only?

**Acceptance criteria.**

- [ ] Hand-written landmark specs ship in `content/landmarks/` and load, reload and draw.
- [ ] With a key, the studio writes new ones that pass `make validate`, and they appear on the
  road on the next run.
- [ ] `parse` rejects or clamps bad specs (tests as for `MonsterSpec`), and placement is
  deterministic with a test.
- [ ] `docs/ARCHITECTURE.md` (§2, §6, §7, §8, §11) and the README layout are updated.

## FR41. Lighting, weather and depth that change with the region

**Status.** **Not started.** Filed 2026-11-21 at the owner's request; the approach is
suggested only.

**Depends on:** [] _(pairs with FR25 and FR29; uses `regions::Scenery` for the per-region knobs)_

**Motivation.** The night is drawn with flat colours, soft light pools and one vignette,
and the four regions differ mostly in which props they place. Light that falls on things,
weather and some depth behind the road would make each region feel like its own place.

**Proposed approach.** *(suggested, not decided)*

- **A lighting pass.** Draw the world into a render target, then composite it through a
  macroquad `Material` (a small fragment shader) with a light map built from
  `Decoration::lights`, the flasks and the player's lantern: warm falloff, a little bloom on
  `LANTERN` and `LAB_GREEN` things, and the night darkness from the light map instead of the
  fixed vignette. A fallback to today's drawing if the shader fails to compile.
- **Weather per region,** as `Scenery` knobs: drizzle and puddle ripples in the forest, drifting
  seed fluff over the fields, chimney smoke in the village, low fog in the castle grounds. All
  on the game clock (invariant 8); flashes such as lightning honour reduce flashing (invariant 9).
- **Depth.** One or two parallax treeline layers at the screen's edges, and lake water that
  moves (ripple bands and a moving moon reflection).

**Open questions.**

- Is a shader acceptable on every machine the game targets (WSL, older GPUs), or should all
  of this stay in plain primitives?
- Which regions get which weather, and does weather ever change gameplay? (Suggested: never;
  it's cosmetic under invariant 7.)

**Acceptance criteria.**

- [ ] Light from lampposts, windows, flasks and the player visibly falls on the ground and
  props, and the game still draws correctly without the shader.
- [ ] Each region has its own weather or atmosphere, shown in `JEKO_SHOT` screenshots per region
  (`region:<n>`).
- [ ] Reduce flashing turns off every new flash, and pausing freezes the weather.
- [ ] Frame time on the default window stays within today's budget.
- [ ] `docs/ARCHITECTURE.md` §7 (draw order, render targets, the `gfx` vocabulary) is updated.

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

**Status.** **In progress: milestones A and C built, B built for monsters only
(2026-11-21).** Done: `openclaw/` with three skills (`jeko-dialogue`, `jeko-monsters`,
`jeko-playtest`) and a README with the agent config; `dialogue::check_bank`, which
`gen_content` runs before it writes `assets/dialogue.json`; `content::vet` (the one word
blocklist; `studio::vet_text` re-exports it); and the offline `validate_content` binary
with `make validate`, which checks the dialogue bank, loads the content dir (monsters,
lesson packs, tips) with the watcher's own scan, and holds each monster's text to the
blocklist. `make validate` is ok and the 165 library tests pass. **Checked against
OpenClaw 2026.10.5** in a throwaway state dir (`OPENCLAW_STATE_DIR` and
`OPENCLAW_CONFIG_PATH` pointed at `/tmp`, so the owner's config was untouched) for the
dialogue skill only: `skills.load.extraDirs` lists it as ready, `agents add` and the
`agents.entries` patch validate, and `cron add` accepts the flags. **Not checked:** an
agent run end to end (needs a model for OpenClaw and a Cerebras key), the weekly cron
against a running Gateway, and the `jeko-monsters` and `jeko-playtest` setup steps
against a real install. The `JEKO_DEV` scenarios in `jeko-playtest` were spot-checked
(one was run); the rest are untested. **Left:** (1) run each skill once through a real
Gateway and fix what's wrong; (2) the run log (`content/runs/<date>.jsonl`) and the
owner's call on its fields; (3) `jeko-lessons`, now that FR22 is in this checkout (the
validator already loads lesson packs and tips); (4) the live bridge and `jeko-villain`
(milestone D; FR23 is built, so it's unblocked); (5) `jeko-tutor` (E, needs the child-data decision). **Next
step:** step (1). **Corrections to this entry** from the docs and the CLI: agents are
`agents.entries.<id>`, not `agents.list[]` (that is a legacy roster); cron is
`openclaw cron` / `openclaw automations`, stored in the Gateway, not in `openclaw.json`;
`cron add` can also run a plain shell `--command` with no agent, which would suit a job
that needs no judgement. Check the OpenClaw details below (endpoints, config keys, skill
layout) against [docs.openclaw.ai](https://docs.openclaw.ai) before relying on them;
they were read on 2026-11-21 and OpenClaw changes quickly.

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
  config: one agent per job (`agents.entries.<id>`), each limited to its own skill and
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

- [x] With OpenClaw absent, the game builds, runs and plays exactly as before, with
  and without `CEREBRAS_API_KEY`. *(Nothing in `src/` references OpenClaw; the one
  game-side change is `dialogue::check_bank`, which only the offline tools call.)*
- [ ] `openclaw/` holds a skill per agent and a README with the config and setup
  steps, and following it from a clean OpenClaw install gets the dialogue agent
  working. *(Skills for dialogue, monsters and playtest and the README are written and
  the dialogue setup steps were checked in a throwaway profile; a full agent run is
  unconfirmed. Lessons, villain and tutor are not built.)*
- [ ] The dialogue agent refreshes `assets/dialogue.json` on request and on a
  schedule, and its output passes validation. *(The skill and the validation are built;
  the first real run with a key is still to do.)*
- [ ] Monster and lesson agents write files the game loads live, through the same
  validation as FR21's specs, and a bad file never crashes the game.
- [ ] The game writes a run log the agents can read, with only the fields the owner
  approved.
- [ ] If the live bridge is built, it's off unless `JEKO_AGENT_URL` is set, never
  blocks the frame, and failures are silent apart from a dev-mode toast.
- [ ] `docs/ARCHITECTURE.md` (*Content, configuration and dev hooks*: the agent dirs,
  run log, env vars, validator; *The question pipeline* if any question path changes)
  and the README layout are updated. *(Done so far: the binaries table, the blocklist
  module, the dialogue rule, the OpenClaw paragraph and the test row. The run log, env
  vars and live bridge come with B and D.)*

---

## Completed

When an entry is fully built, delete it above and add one line here, newest
first: `FR<n> — title — date — why it closed (commit)`. Its number stays retired.

- FR23 — The ultimate villain: a mad scientist who's always up the road, live-scripted by Cerebras —
  2026-11-21 — all three milestones built. **A:** `game/villain.rs`, the doctor and their cart at the top
  of the road, every behaviour, cues from game events, the speech bubble and `assets/villain_lines.json`.
  **B:** `llm/villain_gen.rs` (the prompt, `parse_reply` with unknown enums defaulted, lines cut at a word,
  blocked words and links turned down), `CerebrasClient::request_villain_async` (checked on its thread,
  tagged `RequestKind::Line`), and `game/villain_voice.rs` (`Voice`: one call out, the third lane behind
  questions and look-ahead, `JEKO_VILLAIN` / `JEKO_VILLAIN_RPM`, a flask's two reactions in one call so the
  doctor answers on the same frame, replies routed to the cue they were for by number, late ones dropped);
  a due cue waits up to 2 s for its line, else the bank; each call is a `Line` circle on the creation bar
  and a `line` row in the token panel; the `villain_chatter` setting. **C:** a scheme every 300 m (name,
  a topic for the level, a cosmetic gadget: fog, bubbles, a banner or a glowing flask), whose topic goes to
  the next 3 flasks through `request_task`, with themed look-ahead so they're ready and offline questions on
  every topic; it ends after 3 flasks or 250 m and the doctor says so; while the studio builds a monster
  they're at the bench, and they `Unveil` it by name when it lands. Invariant 7 notes the scheme's topic
  (the owner's call). Dev events `say:<cue>`, `scheme:<topic>`, `nochatter`, and `villain:scheme`,
  `:foiled`, `:creation`. 24 new tests (`villain_gen` 8, `villain_voice` 7, `villain` 8,
  `prefetch` 1); 259 library tests pass, and `make validate` is ok. Checked by screenshot with the stand-in model: a live first sighting, the
  right and wrong reactions from the prefetched pair, a live scheme, each gadget, a creation, a scheme
  steering a flask offline and fizzling out past its span, chatter off, `JEKO_VILLAIN=off`. **Found while
  checking:** a wrong answer's hurt used to replace its reaction (fixed: the reaction is cued after the
  damage at the same rank). **Not checked:** the prompt against the real model at length (the trial key
  answered most calls with 429, which fell back to the bank as designed); the doctor shares the top band
  with the goalpost badge and creation bar, which can cover the bubble on a small window. **Left out:** the
  showdown (FR35). Provisional answers in FR16. (uncommitted)

- FR27 — A discovery map on M, regions, and a castle at the end of the road — 2026-11-21 — built in
  three milestones. **A:** the map (`game/map.rs`, `ui/map_panel.rs`, `ui/minimap.rs`), which pauses the
  game and shows landmarks plus flasks. **B:** `game/regions.rs`, the one table of regions (The Dark Forest,
  Tallybrook Fields from 900 m, Tallybrook Village from 1,100 m, The Castle Grounds from 1,400 m), each with
  a name, a map colour and `Scenery` knobs that `decor::region_scenery` turns into thinner pines and graves
  and more cottages, haystacks, wells, lampposts and columns (the forest is left exactly as it was);
  `world::region_borders_in_chunk` puts a `Kind::RegionSign` and a `GoalKind::Region` goalpost at each border;
  `Game::watch_region` shows an "Entering ..." toast once per run; the map has a band and name for each
  region entered, and names further up stay in the fog. **C:** the road is 1,500 m long (`CASTLE_M`).
  `game/castle.rs` draws a curtain wall, gatehouse and portcullis with the manor's keep behind it
  (`manor::draw_keep`), the manor across the lake fading out as it nears; `clamp_to_path` stops at the gate,
  nothing is placed in the last 20 m or past the wall, no stop or Titan falls at the castle, and the doctor
  ducks inside as you near. `GoalKind::Castle` is the tracker's top-ranked goalpost and the castle is on the
  map from the first step, in the fog, with the metres to go. Standing at the gate calls `Game::win`: the gate
  rises and `ui/victory_panel.rs` shows the run's stats and "Walk the road again". Dev events `tocastle`,
  `atcastle` and `region:<n>[:<m>]`. about 25 new tests (regions, map, decor, world) and compile-time checks on the castle's proportions; checked by screenshot
  (the arrival, the panel, the village, the fields, the map). **Owner's answers:** 1,500 m and the road ends;
  a win screen on arrival; the castle is the manor up close; regions in a const table, names hidden in the
  fog. **Left out, and why:** a showdown with the doctor (FR23's; the panel says "to be continued"),
  persistence of the map (nothing persists yet). Provisional answers in FR16. (uncommitted)
- FR28 — Safe stops along the road: a little house to rest at — 2026-11-21 — built in one pass
  with FR17. `world::safe_stop_for_chunk` places a stop every 250 m (the first at 210 m, each 40 m
  short of a round mark so the 500 m Titan's has room to heal), suppressing the flask in its chunk
  and the encounters in it and beside it. The road opens onto a clearing on the woods side
  (`stop_bulge` in `walkable_x`, a gap in the fence, `draw_stop_pad`), furnished by
  `decor::stop_layout` (cottage, campfire, stumps, lamps, `RestSign`). Inside a stop the curse lifts like
  the hub's, hearts mend 4× faster (`Health::tick_rate`) and `damage_player` ignores hits. A stop is a
  `GoalKind::Stop` goalpost and a map and minimap landmark, with a welcome toast and `tostop`,
  `nearstop` and `shop` dev events. Tests in `world.rs`, `curse.rs` and `health.rs`; checked by
  screenshot. **Owner's answers:** a house, not a mouse; lift the curse, slow heal, hub is the respawn
  (no checkpoint); an empty cottage whose door is the shop. **Left out, and why:** a keeper character
  and a checkpoint, both declined by the owner; the "rest" run-stats prompt, which was optional.
  Provisional answers in FR16. (uncommitted)
- FR17 — Gears as collectible coins — 2026-11-21 — built with FR28. The beach "shell" became a
  brass gear dropped from the mad scientist's lab (owner's pick): `world::gears_for_chunk` lays trails
  of 3 to 5 in about a third of chunks (clear of flasks and zones), `World::collect_gears` takes
  them on touch with a `gfx::sparkle_burst`, they bob and spin in `draw_gear`, and the count
  (`RunStats.gears`, reset with the run) shows in the HUD and Game Over panel. They're spent in the
  safe stop's shop (`game/shop.rs`, `ui/shop_panel.rs`, `Game::buy`): hot soup (4, mends two hearts) and a
  heart charm (12, one more heart up to 8, `Health::add_heart`). Tests in `world.rs`, `shop.rs` and
  `health.rs`; checked by screenshot (the trail, the shop). Provisional prices and rates in FR16. (uncommitted)
- FR33 — A live "creation" bar: show Cerebras generating monsters, questions and lines in real time —
  2026-11-21 — all three milestones built. **A:** `llm/activity.rs` (`GenActivity`), `ui/creation_bar.rs`,
  the `show_creation_bar` setting and `JEKO_DEV=gen:<kind>`, with a circle for each live flask and battle
  question. **B:** each look-ahead request (`Prefetch::tick` and `poll`) is a circle, and the bar prints the
  real stock under the circles ("ready: 9 questions · 2 monsters": the buffer's size and the unseen
  generated monsters for the tier ahead). **C:** the studio reports `StudioEvent`s (started, finished and
  what it made) that `Game::sync_creation_bar` turns into circles, so a monster, a lesson pack and Pebble's
  tips each show, and a made monster's circle is renamed to its name; `RequestKind::Line` has its icon and
  `gen:line`. In a battle the bar keeps only the battle's question. Failed look-aheads and turned-down
  studio jobs are taken off quietly. Tests for each piece (189 pass); checked by screenshot with the
  stand-in model and a throwaway content dir (circles writing and closed, "+N", the stock line, no overlap
  with the toast). **Not built, and why:** nothing opens a `Line` circle yet, because the villain's lines are
  canned until FR23 B (it should `activity.start(RequestKind::Line, ..)` per request); the streaming ring,
  because `CerebrasClient` has no streaming mode and the stage ring was the owner's provisional choice; and
  the monster silhouette preview in the circle, which is optional and would need the monster rig drawn
  inside egui. **Provisional answers, in FR16:** stage ring, `Fallback` shown quietly as "bank", the bar
  stays up while a flask problem is open, the name shows only in the hover once made. Hover detail shows
  the name and time, not tokens. (uncommitted)
- FR22 — Background-generated education content (lesson packs, explanations, field guide) —
  2026-11-21 — built in one pass on FR21's studio and FR32's buffer. **Checked answers:**
  `llm/expr.rs` (a small pure evaluator, 5 tests with hostile input) and `verify`: a
  generated problem is dropped unless its `answer` equals its `expression`, for lesson
  packs, look-ahead questions and live ones, and the compiled-in bank was given expressions
  and explanations and is tested the same way. **Packs:** `Job::Lessons` writes
  `lessons/<topic>-<level>.json` (verified problems only, never overwriting); the watcher
  loads them into the question buffer (`push_unique`), where `request_task` takes them;
  `plan_lesson` asks when the level's buffer runs thin. **Explanations:** `MathTask`
  gained `explanation` and `expression` (both defaulted), and a wrong answer shows a worked
  card beside the next question. **Themed battles:** a spec monster's `theme.topic` goes into
  the question (strict in the buffer, in the live prompt and for the bank). **Adaptive
  practice:** `Progression` counts accuracy per topic and remembers the last four misses;
  the studio aims packs at the weakest topic and writes similar practice for a missed
  problem. **Field guide:** `game/field_guide.rs`, `ui/field_guide_panel.rs` (G or settings),
  a card per built-in (hand-written blurb and true fact) and per spec, unlocked by a win.
  **Pebble's tips:** `Job::Tips` writes `tips/<topic>.json` for a topic that stays weak, and
  Pebble says them in the hub. Tests for each piece; checked by screenshot (the explanation
  card, the field guide) and offline end to end with the stand-in model (wrong answers in a
  themed battle produced similar-practice packs, a tips file, and Pebble's queued tips).
  **Not checked:** any of the new prompts against the real model (the sandbox had no
  network), or a flask opening from a real pack without a wait. The live prompt now asks for an
  `expression`, and a reply without a checkable one falls back to the bank, so a model that
  ignores the instruction would turn every question into a bank question. Provisional
  answers to its open questions are in FR16.

- FR21 — Generated monsters, hot-reloaded into the running game — 2026-11-21 — built in
  three steps. A: monsters as data (`MonsterSpec`, `parse` and its hostile-input tests,
  the tier table, `monster_rig`, `Species`, the polling watcher, the live-swap rule,
  four hand-written specs in `content/monsters/`). B: the studio (`content/studio.rs`,
  `content/prompts.rs`): one thread, lowest lane behind live questions and look-ahead,
  6 requests a minute and 60,000 tokens a session, `finish_monster` forcing the tier,
  limiting the special, vetting the text and parsing before an atomic write of
  `monsters/gen-<name>.json`, three unseen monsters kept in stock for the tier ahead,
  `RequestKind` tags and the token panel's studio line, and a stand-in model so it all
  runs with no key. C: spec monsters in zones (`pick_spec`, `SPEC_SHARE` 40%, never a
  boss, from the roster frozen at the start of the run: the owner's answer to the
  invariant 3 question), a "NEW SPECIES!" tag on a studio monster's first battle, and
  `gen_content --monsters N` / `make gen-monsters`. Tests for the studio, the prompt,
  the binding and its determinism; checked by screenshot (a spec monster blocking the
  road, a generated creature in a battle). **Not done, and not checked:** the studio's
  prompt against the real model (the sandbox had no network), and the `reload` dev
  event (not needed: the watcher polls). Its provisional answers are in FR16.

- FR31 — A goalpost tracker at the top centre — 2026-11-21 — built in one pass:
  `World::next_goalpost` (with the pure `goalposts_in_chunk` and `pick_goalpost`, 7
  tests: deterministic, always north, never a solved flask or beaten monster, the
  bigger event ranks first, a blocker counts down to the line it holds) feeds
  `game/goalpost.rs` (`Tracker`, 7 tests for the pop, unroll, countdown, pulse,
  "Now!", close and hide), painted by `ui/goalpost_panel.rs`; the toast steps down
  below it. New dev events `toroad`, `neargoal`, `atgoal`. Checked by screenshot:
  the circle, the pill with its ring, "Now!". Provisional answers to its open
  questions are in FR16. FR27's regions and castle and FR28's stops each added a `GoalKind` (`Region`, `Castle`, `Stop`); FR29's obstacles will add one when they land

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
