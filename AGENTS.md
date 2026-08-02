# AGENTS.md

Rules for any agent working in this repository. Read this before touching code.

## What this repo is

A personal fork of `xai-org/grok-build`, the Rust source of the `grok` CLI/TUI.

Facts that shape every rule below:

- Upstream is a publish-only mirror. Its history is a series of squashed
  `Synced from monorepo` snapshots, and `CONTRIBUTING.md` states that external
  pull requests are not accepted. Nothing we write will ever go upstream.
- Each snapshot can touch thousands of files. A merge conflict happens only
  where *we* also changed a file. Our merge cost is therefore proportional to
  our delta, not to the size of the upstream change.
- `SOURCE_REV` records the monorepo commit the tree came from.

**The golden rule: keep our delta as small and as boring as possible.**

## Branches and syncing

| Branch | Meaning |
| --- | --- |
| `main` | Our customized tree. Default branch. Carries the delta as a short stack of commits on top of upstream. |
| `upstream` | Exact mirror of `xai-org/grok-build@main`. Never commit here. |

To absorb a new upstream snapshot, run `overlay/scripts/sync-upstream.sh`. It
refreshes the mirror, tags a rollback point `pre-sync/YYYY-MM-DD` on `main`,
enables `git rerere`, and rebases our commits onto the new snapshot.

Because we rebase, publishing `main` needs `git push --force-with-lease`.
Never force-push without a `pre-sync/*` tag pointing at the old `main`.

Keep the stack rebase-friendly: one focused commit per customization, message
prefixed `overlay:`, and no commits that mix upstream fixups with our features.

## Where custom code goes

```
overlay/                  <- 100% ours, upstream never writes here, never conflicts
  overlay-core/           <- Rust package: the single entry point
  scripts/                <- sync-upstream.sh, overlay-diff.sh
  TOUCHPOINTS.md          <- ledger of every edit we made inside xAI's tree
crates/, prod/, third_party/, bin/   <- xAI's tree. Treat as read-mostly.
```

Rust packages we own are named `overlay-*` and live in `overlay/`. The prefix
makes our code obvious in imports, logs, and `cargo` output.

## The escalation ladder

Always try to solve the task at the lowest rung that actually works. Higher
rungs are allowed — some changes genuinely need code — but they cost merge pain
forever, so they need a reason.

1. **Configuration.** `~/.grok/config.toml`, env vars, CLI flags, agent
   profiles, `AGENTS.md`-style instructions. Zero delta.
2. **Hooks, MCP servers, plugins, subagents.** Grok Build ships real extension
   surfaces: `xai-grok-hooks` (pre/post tool, subagent lifecycle, and more),
   `xai-grok-mcp`, `xai-grok-plugin-marketplace`, `xai-grok-subagent-resolution`,
   and ACP. Hooks are external processes, so they can be written in bash,
   Python, or Node and need no Rust and no rebuild. Zero delta.
3. **Overlay package plus a minimal touchpoint.** Put the logic in an
   `overlay-*` package and call it from xAI's code. Prefer a single unconditional
   line at an existing call site; prefer an existing composition root over a new
   one. Add the file to `overlay/TOUCHPOINTS.md`.
4. **A real edit inside xAI's code.** Permitted when nothing else works. Keep it
   as contained as the change allows, explain it in `TOUCHPOINTS.md`, and give it
   its own commit so a bad rebase can be dropped in isolation.

When you find yourself on rung 4, first check whether upstream already exposes a
seam you missed. `rg` for `install(`, `register`, `hook`, `provider`, and
`fn_ptr`-style indirection before you start editing.

## Editing xAI's files

- Prefer adding lines over changing existing ones; prefer appending to the end
  of a list over inserting in the middle.
- Never reformat, re-sort, rename, or "clean up" upstream files. Do not run
  `cargo fmt --all`; format only our own packages.
- Never bump upstream version numbers or edit `SOURCE_REV`.
- Do not delete upstream code to disable it; make the overlay skip it instead.
- If you must edit inside a function, keep the edit on its own line so the
  conflict hunk stays tiny.

`Cargo.lock` is shared with upstream and will move whenever our dependencies
change. On conflict, take upstream's version wholesale and re-run
`cargo check -p xai-grok-pager-bin` to regenerate our entries.

## The touchpoint ledger

`overlay/TOUCHPOINTS.md` lists every file outside `overlay/` that we changed,
why it had to be there, and what would let us retire it.

```sh
overlay/scripts/overlay-diff.sh     # prints the delta, fails on undocumented files
```

Run it before finishing any task that touched xAI's tree, and after every sync.

## Building and verifying

Day to day, run the fork through `grk` (a symlink in `~/bin` to
`overlay/scripts/grk`). It runs the last release build from any directory,
and `grk --rebuild` recompiles first:

```sh
grk                        # run the local build in the current directory
grk --rebuild "fix this"   # rebuild, then run
```

`grk` never rebuilds on its own. Upstream's `build.rs` points
`rerun-if-changed` at a path that does not exist inside the crate, so cargo
re-runs the build script and relinks on every invocation — about 17s even with
no changes. Instead, `grk` prints a one-line warning when the binary is older
than anything in `overlay/` or `crates/`.

A full workspace build is slow (~100 packages). Target packages explicitly:

```sh
cargo check -p overlay-core
cargo test  -p overlay-core
cargo check -p xai-grok-pager-bin      # the crate that links the overlay
cargo build -p xai-grok-pager-bin      # binary lands at target/debug/xai-grok-pager
cargo run   -p xai-grok-pager-bin      # launch the TUI
```

After a sync, verify exactly this set: the overlay packages, every package named
in `TOUCHPOINTS.md`, and one build of the binary. That is what catches an
upstream API that changed under us.

Building from source needs `dotslash` on `PATH` (see `README.md`).

## Herdr

[Herdr](https://herdr.dev) is the terminal multiplexer we run this build in. It
keeps each agent in its own pane and tracks whether it is idle, working, or
blocked. The server runs as a Homebrew service, so it survives logout.

One piece makes the fork work there, inside `overlay/` and therefore free of
upstream touchpoints: `grk` execs through a symlink named `grok` that it keeps
next to the binary. Herdr identifies an agent from the basename of the path the
process was exec'd from, and upstream builds the artifact as `xai-grok-pager`
while every Herdr manifest expects `grok`. Launched under its own name the pane
is reported as `unknown`; launched through the symlink it is a first-class
`grok` agent, with no environment hints involved. `HERDR_AGENT=grok` stays
exported only as a fallback for when the symlink cannot be created.

Do not try to mirror Grok's native in-process subagents into panes: a native
subagent has no terminal of its own, so a pane fed from `SubagentStart` or
`PreToolUse` can only ever show a flat log of tool calls, never a Grok window.
Grok already renders that child transcript itself, in a framed fullscreen view
reachable with Enter on the scrollback block or Ctrl+G. The pattern Herdr is
built for is the opposite one: the agent in the current pane starts *real*
agents — separate processes, each with its own Grok UI — in sibling panes.

`overlay/hooks/herdr-agent-panes.json`, symlinked into `~/.grok/hooks/`, makes
that the only option here. It is a `PreToolUse` hook on `spawn_subagent` that
denies the call and returns the pane recipe as the deny reason, so the model
gets the redirect exactly when it needs it instead of relying on a rule it
might not recall. It allows the call when `HERDR_PANE_ID` is unset or `herdr`
is missing, which keeps native subagents working outside Herdr, and it uses
only `jq` and Herdr's own CLI. Hooks load at session start, so a fresh session
is needed after editing it.

The deny reason is the single source of truth for how to drive a pane agent —
do not restate the commands here, or the two copies will drift apart. Every
step in it was a failure first, so change it only against a live Herdr.

What the recipe is worth knowing about: a pane agent is an ordinary Grok
session, so it keeps its memory across turns and can be asked follow-ups. The
parent owns its lifetime and closes the pane when it is done with the agent;
nothing closes it automatically, because a one-shot pane throws away a session
that can still answer. Closing also deletes the agent's session: a pane agent
is a top-level session and would otherwise show up in `grok sessions list` and
the `/resume` picker forever, which native subagents never do — their sessions
exist on disk but are kept out of that list. Delegating this way is not free
either: each agent is a
separate process with its own context and token spend, it has to be prompted
and awaited over the CLI, and its result comes back as scraped screen text
rather than a value. There is also no depth limit, unlike native subagents: an
agent in a pane can open panes of its own.

Driving Herdr from inside a session is Herdr's own job, not ours: it ships an
official agent skill, installed with
`npx skills add herdrdev/herdr --skill herdr -g`. It lands in
`~/.agents/skills/herdr`, which Grok scans, and it teaches the agent to split
panes, start helper agents in sibling panes, read output, and wait on state.
Do not write local rules or wrapper scripts for that; the skill is the source
of truth and it activates only when the user mentions Herdr.

Herdr's own `agent start --kind grok` launches whatever `grok` resolves to on
`PATH`, so `~/bin/grok` is a symlink to `overlay/scripts/grk` and the official
binary that the installer put in `~/.local/bin` was deleted. `~/bin` comes
first on `PATH`, so every `grok` on this machine — ours, Herdr's, any script's
— is this fork. Recreate the symlink after cloning:

```sh
ln -sfn "$PWD/overlay/scripts/grk" ~/bin/grok
```

On a tree with no `target/release` build yet, the first `grk` compiles, which
takes far longer than the 30-second startup timeout of `agent start`. Build
once by hand there; after that `grk` never builds on its own, so agent panes
start immediately.

Use `herdr agent explain <pane>` when a pane shows the wrong state; it prints
the manifest, the rule that matched, and the evidence.

## Definition of done

A change is finished when all of these hold:

- It sits at the lowest workable rung of the ladder.
- Any new upstream touchpoint is recorded in `TOUCHPOINTS.md`.
- `overlay/scripts/overlay-diff.sh` passes.
- The overlay packages and the binary compile, and overlay tests pass.
- Our commits are separable and prefixed `overlay:`.
