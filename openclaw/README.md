# OpenClaw agents for JEKO (FR30)

[OpenClaw](https://docs.openclaw.ai) is a self-hosted agent gateway. This folder holds
JEKO's skills for it, one per job. **The game never needs OpenClaw.** It builds, runs
and plays the same with no OpenClaw installed, and nothing in `src/` calls it. An agent
only runs the repo's own tools (`make gen-content`, `make validate`, the game's dev hooks) and leaves the
result in the working tree for you to review.

| Skill | What the agent does | Status |
|---|---|---|
| [`jeko-dialogue`](skills/jeko-dialogue/SKILL.md) | Regenerates `assets/dialogue.json`, validates it, shows the diff | Built (milestone A) |
| [`jeko-monsters`](skills/jeko-monsters/SKILL.md) | Designs monster specs into `content/monsters/`, checks them with `make validate` | Built (milestone B, monsters only) |
| [`jeko-playtest`](skills/jeko-playtest/SKILL.md) | Runs `JEKO_DEV` / `JEKO_SHOT` scenarios, looks at the screenshots, reports | Built (milestone C) |
| `jeko-lessons` | Writes lesson packs into the content dir | Not built (milestone B, lessons) |
| `jeko-villain`, `jeko-tutor` | Need FR23 and a decision on child data | Not built (D, E) |

Checked against OpenClaw 2026.10.5 and its docs (2026-11-21). OpenClaw changes quickly, so
if a command below stops working, `openclaw <command> --help` is the source of truth.

## Setup (milestone A: the dialogue agent)

You need the JEKO dev shell working (`make test` passes) and `CEREBRAS_API_KEY` set in the
checkout's `.env` or secretspec (see the README). Then:

1. **Get OpenClaw and start the Gateway.** The flake has an opt-in shell with the version
   this was checked against: `nix develop .#openclaw` (nixpkgs marks it insecure, see
   *Safety*). Or install it yourself (docs.openclaw.ai). Then `openclaw onboard`,
   `openclaw gateway install` and `openclaw gateway start`.
2. **Make the skill visible** by adding the repo's skills folder as an extra skill
   directory. Replace the path with your checkout:

   ```bash
   openclaw config patch --stdin <<'JSON5'
   { skills: { load: { extraDirs: ["/path/to/JEKO/openclaw/skills"] } } }
   JSON5
   openclaw skills check          # jeko-dialogue should be listed as ready
   ```

   The skill finds the repo from its own location (`{baseDir}/../../..`), so keep it in
   place. Installing a *copy* with `openclaw skills install` would break that.
3. **Add a locked-down agent** that can read files and run commands but not write files
   itself (the generator does the writing):

   ```bash
   openclaw agents add jeko-dialogue --non-interactive --workspace /path/to/JEKO
   openclaw config patch --stdin <<'JSON5'
   {
     agents: { entries: { "jeko-dialogue": {
       skills: ["jeko-dialogue"],
       tools: { allow: ["read", "exec"], deny: ["write", "edit", "apply_patch"] },
     } } },
   }
   JSON5
   ```
   `agents add` seeds `IDENTITY.md`, `SOUL.md` and `USER.md` into the workspace, so in the
   repo they appear as untracked files. `.gitignore` already lists them.
4. **Ask it** (the Control UI is at `openclaw dashboard`, or from a terminal):

   ```bash
   openclaw agent --agent jeko-dialogue --message "Refresh JEKO's NPC dialogue."
   ```

   It runs `make gen-content` and `make validate`, reads the lines, and shows you the diff.
   Review it, rebuild to hear the lines in the game, and commit it yourself.
5. **Optional: a weekly refresh.**

   ```bash
   openclaw cron add "0 9 * * 1" "Refresh JEKO's NPC dialogue." \
     --name "JEKO dialogue refresh" --agent jeko-dialogue --session isolated
   ```

   A scheduled run rewrites `assets/dialogue.json` in whatever checkout the agent's
   workspace points at, and refuses if the file already has uncommitted changes. Point
   the agent at a separate clone (`git worktree add ../JEKO-agent`) so a Monday run never
   lands in the middle of your own work.

## The other agents

Same pattern as the dialogue agent: add the skill's folder (already covered by the
`extraDirs` entry above), add an agent, limit its tools.

```bash
openclaw agents add jeko-monsters --non-interactive --workspace /path/to/JEKO-agent
openclaw agents add jeko-playtest --non-interactive --workspace /path/to/JEKO-agent
openclaw config patch --stdin <<'JSON5'
{
  agents: { entries: {
    // Writes spec files, so it needs `write`. It may write anywhere in its workspace,
    // so give it a separate worktree (git worktree add ../JEKO-agent) and review the
    // result with `git status`, then copy what you like back.
    "jeko-monsters": {
      skills: ["jeko-monsters"],
      tools: { allow: ["read", "write", "exec"], deny: ["edit", "apply_patch"] },
    },
    // Reads screenshots, so it needs the image tool. It writes nothing but
    // screenshots, and the skill points those at a scratch folder.
    "jeko-playtest": {
      skills: ["jeko-playtest"],
      tools: { allow: ["read", "exec", "image"], deny: ["write", "edit", "apply_patch"] },
    },
  } },
}
JSON5
```

Check the tool names with `openclaw agents --help` and the docs: they change.

```bash
openclaw agent --agent jeko-monsters --message "Design three new monsters, one each for tiers 1, 2 and 4."
openclaw agent --agent jeko-playtest --message "Playtest the game and report."
```

The monsters agent writes into a **worktree**, not the checkout you're playing in: the
game loads whatever is in its own `content/monsters/`. To see them, copy the files you
like into your checkout (the running game picks them up within half a second) or run the
game from the worktree. To run the playtest after your own changes, point its workspace
at the checkout you changed, since it needs your code, and it writes only to a scratch
folder.

## Model

OpenClaw picks its own model, separate from the game's `CEREBRAS_MODEL`. The generator
itself always calls Cerebras through the game's client, with the checkout's key. Whatever
model OpenClaw uses only drives the agent's reading and reporting. To use Cerebras for that
too, OpenClaw has a Cerebras provider (`openclaw onboard --auth-choice cerebras-api-key`).

## Safety

- nixpkgs marks OpenClaw insecure: it gives a model full system access by default and
  reads untrusted content, so it can be prompt-injected. That is why the flake keeps it
  out of the default shell. The agent above is limited to `read` and `exec`, but `exec`
  runs any command in its workspace, so give it a checkout with nothing sensitive in it.
- Install only this repo's skills for these agents, not third-party ClawHub ones.
- Keep the Gateway on localhost. The agents need no inbound access.
- The skill is told never to read `.env`. The agent's `exec` tool can still read files in
  its workspace, so keep keys out of it: secretspec (the README's first option) does that.
- Generated lines go through `dialogue::check_bank` (length, count, word blocklist) and
  then the agent's own read-through. A person still reviews the diff before it ships,
  because the game is for children and a blocklist is a floor.

## Checking a skill's output without OpenClaw

`make validate` is the same check the agent runs. It's offline and needs no key:

```bash
make validate                                        # dialogue bank and content dir
cargo run --bin validate_content -- --dialogue FILE  # one bank
cargo run --bin validate_content -- --content DIR    # one content dir
```
