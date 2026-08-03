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

Cargo never collects garbage: every rebuild adds artifacts and removes none, so
`target/` only grows. It reached 25 GB here, against a 160 MB binary. Two
directories hold almost all of it, and both are caches that a build rebuilds on
demand:

- `target/release/incremental` — upstream turns incremental on for release
  (`Cargo.toml:341`) to keep local rebuilds fast, at the cost of the largest
  single directory in the tree.
- `target/debug` — `grk` and `--release` never write here; it exists because
  `cargo check` and `cargo test` use the dev profile.

Reclaim them when a task is done, not between the builds inside one:

```sh
rm -rf target/release/incremental target/debug
```

Deleting either mid-task only buys space back by making the next `cargo check`,
`cargo test`, or `grk --rebuild` in that same task recompile from scratch.

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

Delegation stays native, and every native subagent gets its own pane showing
its session live. `overlay/hooks/herdr-subagent-panes.json`, symlinked into
`~/.grok/hooks/`, splits a sibling pane on `SubagentStart` and starts
`grok --resume <subagentId>` in it, then closes that pane on `SubagentStop`.
The hook is inert when `HERDR_PANE_ID` is unset or `herdr` is missing, so
delegation outside Herdr behaves exactly as upstream ships it, and it uses only
`jq` and Herdr's own CLI.

This works because of four properties of upstream, none of which we added:

- The leader multiplexes one session to many clients and fans notifications out
  to every subscriber (`xai-grok-shell/src/leader/server.rs:2255`). Subscribing
  is implicit in `session/load`.
- The first client to load a session keeps the driver role
  (`leader/server.rs:1854`), and a child session inherits its parent's driver
  (`:654`, `:2275`), so the watcher is a passive observer and cannot steal the
  subagent from its parent.
- A subagent's session is `hidden` (`session/persistence.rs:1014`), which only
  hides it from the *search* paths — listing and title lookup. Loading it by
  explicit UUID works, which is why `--resume <subagentId>` opens it.
- `SubagentStart` already carries the child's session id as `subagentId`
  (`xai-grok-hooks/src/event.rs:469`).

That first property needs `[cli] use_leader = true` in `~/.grok/config.toml`.
Without a leader the two processes only share a disk and the pane shows a
frozen snapshot. This is a machine-wide change, not a fork-local one: leader
mode is refused when a sandbox profile other than `off` is requested
(`docs/user-guide/18-sandbox.md:156`), and killing the leader takes its hosted
sessions with it. Undo it by deleting the line and running `grok leader kill`.

Three details in the hook are load-bearing, each of them a failure first. The
hook returns in ~130ms and does its work in a process started with `setsid`,
because the dispatcher *awaits* even an observe hook — a slow one stalls the
parent's update loop — and because the runner `killpg`s the hook's process
group on return, which reaches an ordinary background child but not a new
session. It retries `herdr agent start` for ~30s, because `pane split` returns
before the new pane's shell reaches its prompt and starting into it fails with
`agent_pane_busy` until it does. And the agent name comes from the *tail* of
the subagent id: a UUIDv7 opens with a timestamp, so two subagents spawned in
the same millisecond collide on a name Herdr requires to be unique.

The layout is fixed: the parent keeps the whole left half and subagents stack
down the right one. The first subagent splits the parent `right` at ratio 0.5,
every later one splits the newest live watcher `down`, and placement runs under
a `mkdir` lock because two subagents starting together would otherwise both see
an empty right column and open two of them. The right column only exists while
a subagent does — the parent reclaims the full width once the last pane closes.

Finding the pane to split is the part that fought back, and `use_leader` is why.
Hooks run in the process hosting the agent — the leader — not in the client you
are typing into. The leader lives wherever it was first started, so
`HERDR_PANE_ID`, the process tree, and Herdr's focused pane all name some other
pane; here they pointed at a session in an unrelated workspace while the client
sat in another. Herdr exposes no mapping from a Grok session to the pane
showing it (`agent_session_id` is reported as null). What it does expose is
which panes hold a `grok` and with which cwd, so the worker takes the pane whose
cwd matches the session and whose agent has no name — our own watchers are
named `sub-*` — and falls back to the focused pane. Do not "fix" this back to
`HERDR_PANE_ID`.

The pane never outlives its subagent, and `SubagentStop` alone cannot promise
that: it does not fire for an interrupted turn, it does not fire when the
provider kills the turn, and it cannot fire at all if the parent dies. So the
worker keeps watching after it starts the agent. It closes the pane on any of
three markers, none of which appears in a subagent that completed (checked
across 200 local sessions):

- `"outcome":"cancelled"` in the subagent's own `events.jsonl` — the turn was
  interrupted.
- a `retry_state` update of type `failed` in its `updates.jsonl` — the
  provider gave up. The same record type also carries `retrying`, so the
  `type` has to be matched and not just the word. A 401, an exhausted retry
  budget or a tool schema the backend rejects all end here, and nothing is
  ever written afterwards.
- the parent dropping out of `~/.grok/active_sessions.json`, or its pid no
  longer answering `kill -0`.

Verified: killing a parent with `SIGKILL` clears the pane in under 3s, a
subagent on a model that 401s clears it in under 4s, and cancelling the
parent's *turn* leaves a still-running subagent alone until it really
finishes.

Known sharp edges: the watcher is a full ACP client, not a read-only view, so
typing into its prompt injects a message into the subagent's session, and
several subagents at once split the height into unreadable rows.
`GROK_HERDR_KEEP_SUBAGENT_PANES=1` keeps the panes open for inspection.

Starting whole Grok agents in panes — separate sessions with no inherited
context, driven over the CLI — is still possible, but it is not the default:
ask Herdr's own skill for it when a task really wants a peer agent rather than
a subagent.

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
- `target/release/incremental` and `target/debug` are deleted, now that nothing
  else in the task needs them.
