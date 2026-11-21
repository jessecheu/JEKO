---
name: jeko-monsters
description: Design new monsters for JEKO (Math Quest) as JSON spec files in the content dir, check them with the repo's validator, and leave them for the owner to review. Use when asked to make, design or add JEKO monsters or creatures.
metadata: {"openclaw": {"emoji": "🐸", "requires": {"bins": ["make", "git", "nix"]}}}
---

# jeko-monsters

JEKO is a kids' math exploration game. Monsters are fought by answering math problems.
A monster is a JSON file in `content/monsters/`. The running game reloads that folder
every half second, so a good file shows up with no rebuild. You write those files. You
don't edit code, you don't commit, and you never push.

## Where the repo is

This skill lives at `openclaw/skills/jeko-monsters/` inside the JEKO checkout, so the
repo root is three folders up from `{baseDir}`. Every command below runs there:

```bash
cd "{baseDir}/../../.."
git rev-parse --show-toplevel   # must print the JEKO checkout; if not, stop and ask
```

## The format

Read two or three of `content/monsters/*.json` first: they are the examples. A spec:

| Field | Values |
|---|---|
| `id` | 1 to 32 of `a-z 0-9 _ -`. **Must equal the file name** (`id` `fenwick` is `fenwick.json`). |
| `name` | 1 to 16 characters, original. |
| `blurb` | One friendly sentence, at most 80 characters. |
| `fact` | One **true** real-world fact a child would enjoy, one sentence. |
| `theme` | `{ "subject": "math", "topic": "addition" }`. |
| `body` | `{ "shape": blob \| shell \| dome \| segmented \| wave \| spiky_ball, "w": 30..80, "h": 24..70 }` |
| `palette` | `main`, `accent`, `belly`: three `#rrggbb` colours. Too-dark ones get lightened. |
| `eyes` | `{ "count": 1..3, "size": 3..8, "stalks": bool, "look": cute \| grumpy \| sleepy }` |
| `parts` | Up to 6 of `{ "kind": claw \| tentacle \| spike \| fin \| wing \| horn \| leg \| tail, "count": 1..6, "size": 0.5..1.5, "side": both \| left \| right }` |
| `accessory` | One of the values used in `content/monsters/*.json`, or `none`. |
| `idle` | `bob \| scuttle \| float \| sway \| pulse` |
| `attack` | `lunge \| zap \| spin \| slam \| spit` |
| `tier` | 1 to 4: how tough it plays. The game sets all the numbers from the tier; you can't. |
| `special` | `none \| stun \| grows \| steals_hat` |

An unknown enum value rejects the whole file; numbers out of range are clamped. If
unsure of a value, copy one from an existing spec. `docs/ARCHITECTURE.md` section 8
(*Spec monsters*) is the full reference.

## Steps

1. **Look.** `ls content/monsters/` and read the names and blurbs. Don't repeat a name
   or an idea. Aim for variety in shape, palette and tier across a batch.
2. **Write each monster atomically.** Write the JSON to `content/monsters/<id>.json.tmp`
   first (the game ignores that name), then `mv` it to `content/monsters/<id>.json`, so
   the game never reads half a file.
3. **Check.** Run `make validate`. It must print `ok` for the content dir and exit 0.
   If it names your file, fix that file and run it again; two fixing rounds at most,
   then delete the file you couldn't fix and report it.
4. **Read it yourself.** These are for children. Each name, blurb and fact must be
   warm, original (no characters or brands from other games, films or shows), not
   scary and not violent, and the fact must be true. The word check is a floor.
5. **Show what you made.** List each new file with its name, tier, special, blurb and fact.
6. **Stop.** Leave the files uncommitted. Tell the owner that a running game has already
   loaded them (fight one with `JEKO_DEV=battle:<id>@30`), that `git status` lists them,
   and that deleting a file removes the monster.

## Rules

- Write only `content/monsters/<id>.json`. Never overwrite or edit an existing monster
  unless asked to by name. Don't touch code, `assets/` or other content folders.
- If `.env`, secrets or keys come up, don't read, print or copy them.
- Don't run `git commit`, `git push` or anything that changes history or a remote.
