---
name: jeko-dialogue
description: Refresh the NPC dialogue lines in JEKO (Math Quest) by running the repo's generator, checking the result, and leaving the diff for the owner to review. Use when asked to refresh, regenerate or rewrite JEKO's NPC dialogue or Professor Pebble's lines.
metadata: {"openclaw": {"emoji": "💬", "requires": {"bins": ["make", "git", "nix"]}}}
---

# jeko-dialogue

JEKO is a kids' math exploration game. Its NPCs (Professor Pebble, Wanderer Finch,
Old Lantern) say one-sentence lines from `assets/dialogue.json`. You refresh that
file. You don't edit it by hand, you don't commit, and you never push.

## Where the repo is

This skill lives at `openclaw/skills/jeko-dialogue/` inside the JEKO checkout, so the
repo root is three folders up from `{baseDir}`. Every command below runs there:

```bash
cd "{baseDir}/../../.."
git rev-parse --show-toplevel   # must print the JEKO checkout; if not, stop and ask
```

## Steps

1. **Start clean.** Run `git status --porcelain assets/dialogue.json`. If it prints
   anything, the owner has unreviewed changes to the file: stop and tell them. Do not
   overwrite it.
2. **Generate.** Run `make gen-content`. It asks Cerebras for 5 lines per NPC and
   writes `assets/dialogue.json` only if the whole bank passes the checks (3 to 12
   lines each, at most 140 characters, no blocked word, Professor Pebble present). A
   rejected bank leaves the old file in place and prints why.
   - If it says `set CEREBRAS_API_KEY`, the checkout has no key. Tell the owner. Never
     read, print, copy or create `.env` or any other secret file.
   - If it fails for any other reason, report the error text as it is. Don't retry in
     a loop: one retry for a network error is enough.
3. **Check.** Run `make validate`. It must end with `ok` for the dialogue file and the
   content dir and exit 0. If it fails, run `git checkout -- assets/dialogue.json` to
   put the old file back and report the failure.
4. **Read the lines yourself.** Open `assets/dialogue.json` and read every line. These
   are said to children. Each must be warm, one sentence, about learning, exploring or
   the game, with no real people, brands or characters from other games. If any line
   isn't, restore the old file as in step 3 and report which line and why. The word
   check is a floor, not a review.
5. **Show the diff.** Run `git diff --stat assets/dialogue.json` and
   `git diff assets/dialogue.json`, and summarise: how many lines changed per NPC and
   two or three examples of the new lines.
6. **Stop.** Leave the change uncommitted in the working tree. Tell the owner that the
   file is compiled into the game, so the new lines show up after a rebuild
   (`make run`), and that `git checkout -- assets/dialogue.json` undoes it.

## Rules

- Touch only `assets/dialogue.json`. Don't change code, other assets or `content/`.
- Don't add or rename NPCs. The game looks up Professor Pebble by name.
- Don't run `git commit`, `git push` or anything that changes history or a remote.
