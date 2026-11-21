# JEKO — AGENTS.md

JEKO (Math Quest) is a 2.5D beach exploration game in Rust (macroquad + egui).
The player walks up an endless boardwalk and answers math problems that Cerebras
generates live. See [README.md](README.md) for the stack, the layout and setup,
and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how the code fits together.

## Where things are

| Want | Read |
|---|---|
| Stack, project layout, setup, build | [README.md](README.md) |
| How the code fits together, and the rules a change has to keep | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Read its **Bottom line** and *Invariants* section first. |
| What to build next / where the last agent stopped | [FeatureRequestToDo.md](FeatureRequestToDo.md) |

## Feature work

[FeatureRequestToDo.md](FeatureRequestToDo.md) is the shared todo list and the
handoff point between agents. Read it before starting feature work, keep the
entry you're working on current while you work (tick its criteria, update its
**Status**), and if you stop partway, write down what's done, what's left, and
the next step in its **Status** line. The rules are at the top of that file.

## Keeping the architecture doc current

[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) describes the design **as built**.
It's only useful while it's true, so treat it like code. **A change that alters the
design updates the doc in the same change.** Don't leave it for a later tidy-up.

Update it when your change does any of these:

- **Modules.** Adds, removes, renames or moves a module, or moves a responsibility
  between modules (*Crate layout and layering*). Update the layout tree in
  [README.md](README.md) too.
- **The frame or `Game`.** Changes the order of the frame loop in `src/main.rs`, or
  adds a mode or state to `Game` that changes what `Game::update` does or in what
  order (*The frame loop*; *`Game`: state owner and modes*).
- **Questions.** Changes `TriggerState`, `CerebrasClient`, the prompt, the fallback
  bank, or how answers are judged and rewarded (*The question pipeline*).
- **The world.** Changes coordinates, path geometry, chunking, the hub, or what gets
  placed per chunk. New placed things like lava, blockers, cages or shells count
  (*World model*).
- **Drawing.** Changes the draw order, the depth sort, render targets, or the shared
  `gfx` vocabulary (*Rendering*).
- **Rules and numbers.** Adds or retunes a monster, battle phase, upgrade or
  progression rule, or changes any number the doc cites (*Battles and monsters*;
  *Player, health and effects*; *Progression and the UI layer*).
- **Panels.** Adds a panel, or changes what a panel reads or writes (*Progression and
  the UI layer*).
- **Content and tooling.** Adds an asset, env var, dev hook (`JEKO_DEV` event), or
  area of tests (*Content, configuration and dev hooks*).
- **Invariants.** Breaks, relaxes or adds an invariant (*Invariants*). These are the
  owner's call. Ask before breaking one, then record the decision there.
- **Known limits.** Builds something listed under *Known limits and open design*, or
  finds a new limit.

How to update it:

- Describe what the code does **now**. Plans, options and open questions go in
  [FeatureRequestToDo.md](FeatureRequestToDo.md), not here.
- Fix the section that's wrong instead of appending a changelog, and check that the
  **Bottom line** is still true.
- Cite code by path and symbol (`game/world.rs`, `encounter_for_chunk`), not by line
  number. Line numbers go stale on the next edit.
- Bump the `timestamp` in the doc's frontmatter.
- When you fold a finished FR into *Completed*, checking this doc against what you
  built is part of that step.
- If you find the doc already wrong, fix it, even if your change didn't cause it.
  If you can't tell what's right, ask the owner.

No update is needed for changes that leave structure and behaviour as the doc
describes them: a bug fix, an internal refactor, art polish, or tuning a constant the
doc doesn't cite.
