---
name: jeko-playtest
description: Playtest JEKO (Math Quest) by running its scripted dev scenarios, looking at the screenshots and reporting anything that looks broken. Use when asked to playtest, smoke-test or check how JEKO looks after a change.
metadata: {"openclaw": {"emoji": "🎮", "requires": {"bins": ["make", "git", "nix"]}}}
---

# jeko-playtest

You run the game headlessly-ish through its two dev hooks, look at what it drew and
say what looks wrong. You change no code and no content, you don't commit, and you
never push. Reports only.

## Where the repo is

This skill lives at `openclaw/skills/jeko-playtest/` inside the JEKO checkout, so the
repo root is three folders up from `{baseDir}`:

```bash
cd "{baseDir}/../../.."
git rev-parse --show-toplevel   # must print the JEKO checkout; if not, stop and ask
```

## The hooks

- `JEKO_SHOT=PATH.png:FRAME` saves a screenshot at that frame, prints a `STATE ...`
  line (`Game::debug_summary()`) to stderr, and exits.
- `JEKO_DEV=name@frame,...` fires scripted events at frames: `hurt`, `kill`, `jump`,
  `roll`, `pause`, `settings`, `map`, `far`, `restart`, `nohud`, `topebble`, `toflask`,
  `tozone`, `e`, `right`, `wrong`, `curse`, `cursefull`, the battles `pinch`, `jelly`,
  `puff`, `coco`, `gull`, `wyrm`, `titan`, and `battle:<id>` for a monster in the
  content dir. Full list: `docs/ARCHITECTURE.md` section 11.
- With either set, time steps a fixed 1/60 s, so a run repeats exactly. With no API
  key, questions come from the offline bank, so the run needs no key and no network.

A run may print `Can't open PCM device` from the audio thread when there is no sound
device; it is harmless, so ignore it. The game does need a display. If a run fails with a window or GL error, say so and stop;
don't try to install anything.

## Steps

1. **Start clean.** `make build`. If it fails, report the compiler error and stop.
2. **Run the scenarios.** Put screenshots in a scratch folder, not the repo (for
   example `/tmp/jeko-playtest/`). Run each as
   `JEKO_SHOT=/tmp/jeko-playtest/<name>.png:<frame> JEKO_DEV=<events> nix develop --command cargo run --quiet`:

   | Name | `JEKO_DEV` | Frame | What should be on screen |
   |---|---|---:|---|
   | `hub` | (none) | 60 | The hub: fire, Professor Pebble, the gate. |
   | `pebble` | `topebble@5,e@20` | 90 | Pebble's dialogue box. |
   | `flask` | `toflask@5,e@20` | 60 | A flask problem with an answer box. |
   | `answer` | `toflask@5,e@20,right@40` | 90 | A solved flask and a toast. |
   | `battle` | `jelly@10` | 60 | A battle with a nameplate and a problem. |
   | `monster-<id>` | `battle:<id>@10` | 60 | One per file in `content/monsters/`. |
   | `map` | `far@5,map@20` | 60 | The discovery map. |
   | `road` | `far@5,nohud@6` | 60 | The road with no panels. |
   | `hurt` | `hurt@10,hurt@12` | 40 | Fewer hearts. |
   | `curse` | `far@5,curse@6` | 120 | The curse's fever look. |

3. **Look at every screenshot** with your image tool, and read each `STATE` line. Judge
   only what's there: text cut off or overlapping, a panel covering the action, an
   empty or black frame, a monster that is blank or a single colour, colours that don't
   read against the background, hearts or numbers that disagree with the `STATE` line.
4. **Report.** One line per scenario: `ok`, or what looks wrong and where in the image.
   Name the screenshot path for each problem. Separate *looks wrong* from *can't tell*.
   Don't guess at causes in the code, and don't call something wrong that you can't see.
5. **Stop.** Don't fix anything. Leave the screenshots in the scratch folder and tell
   the owner where they are.

## Rules

- Write only to the scratch folder. Don't change code, `assets/`, `content/` or settings.
- Don't read, print or copy `.env` or other secrets.
- Don't run `git commit`, `git push` or anything that changes history or a remote.
