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
- `~/bin/grok` is a symlink to the **installed release**, never to this
  checkout:

  ```sh
  ~/bin/grok -> ~/.local/share/grok-overlay/bin/grok
  ```

  It is kept first on `PATH` so every `grok` invocation on this machine
  resolves to the fork instead of the official binary.
  [Conan Code](https://github.com/jattento/coinor) is the IDE that hosts it:
  it spawns `~/bin/grok` directly inside its own terminal tabs and owns the
  pane/window chrome itself, so `grok` here is a harness with no
  terminal-multiplexer layer between it and the IDE.

  Never point `grok` at `target/`, at `overlay/scripts/grk`, or at a git
  worktree. Worktrees are transitory and get deleted; a build tree drifts from
  what every other machine runs and reports an unstamped version. Install a
  published release instead — see "Releasing" below.

- `grk` is the separate *development* launcher for running an uninstalled local
  build. It is a symlink to this checkout, and it never shadows `grok`:

  ```sh
  ln -sfn "$PWD/overlay/scripts/grk" ~/bin/grk
  ```

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

### Always start new work from the latest `origin/main`

Never start a feature on whatever branch happens to be checked out. This repo
accumulates worktrees and stale feature branches, and because we *rebase* onto
upstream snapshots, an old branch can carry commits that look identical to
`main`'s but have different SHAs — so it reads as "ahead" while actually
missing everything landed since it was cut.

Before writing a single line:

```sh
git fetch origin
git log --oneline HEAD..origin/main     # must print nothing
git worktree list                       # who already owns `main`?
```

If the middle command prints anything, you are behind. Do not build on it:

```sh
git switch -c <feature> origin/main
```

`main` is often checked out in another worktree, which makes `git switch main`
fail here. That is not a reason to work on the stale branch: branch from
`origin/main` as above, then land the work by cherry-picking onto `main` in the
worktree that owns it.

Two failures this prevents, both quiet and both already hit once:

- A release cut from a stale branch ships a **downgrade**, dropping every
  overlay commit the branch never had.
- Editing a file that the stale branch has an older copy of (`AGENTS.md`,
  `overlay/TOUCHPOINTS.md`, `Cargo.lock`) and cherry-picking it forward
  **reverts** the newer content along with it.

Same rule for the docs: check `git diff main <branch> -- <file>` before editing
a shared file from a branch you did not just cut.

## Where custom code goes

```
overlay/                  <- 100% ours, upstream never writes here, never conflicts
  overlay-core/           <- Rust package: the single entry point
  scripts/                <- sync-upstream.sh, overlay-diff.sh, install-hooks.sh
  TOUCHPOINTS.md          <- ledger of every *modification* to an upstream file
  delta-budget.tsv        <- per-file changed-line ratchet (M files + adapters)
  adapters-outside-overlay.txt  <- sanctioned thin adapters that must sit in crates/
crates/, prod/, third_party/, bin/   <- xAI's tree. Treat as read-mostly.
```

Rust packages we own are named `overlay-*` and live in `overlay/`. The prefix
makes our code obvious in imports, logs, and `cargo` output.

**Hard rule:** a whole file we author lives in `overlay/`, never inside
`crates/` or any other upstream tree. Parking a new file of ours next to
upstream code is not a touchpoint — it is a misplaced ownership boundary.
A touchpoint is a *modification* to an upstream file (git status `M`), not a
new file of ours (git status `A`) sitting in their tree.

**One sanctioned exception:** a thin adapter that cannot leave the upstream
module tree (ACP dispatch enters through a closed `match`, types are
shell-private, etc.). Every such file must satisfy all three conditions:

1. **Naming.** Basename starts with `overlay_` (mirrors the `overlay-*` crate
   prefix). Prevents collision with a future upstream snapshot.
2. **Listed.** Path is in `overlay/adapters-outside-overlay.txt`.
3. **Budgeted.** Path has a line entry in `overlay/delta-budget.tsv` so the
   adapter cannot quietly grow into business logic.

Naming is not waivable — listing a non-`overlay_*` path does nothing. A listed
path that leaves the delta fails the gate until its line is removed, so the
list cannot rot.

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

`overlay/TOUCHPOINTS.md` lists every upstream file we *modified*, why the edit
had to be there, and what would let us retire it. It is the human ledger for
status-`M` paths only.

`overlay/scripts/overlay-diff.sh` is the machine gate. Against `upstream/main`
it splits the delta and enforces three checks:

1. **Whole files outside `overlay/` only as sanctioned adapters.** Status-`A`
   paths outside `overlay/` must be `overlay_*`-named, listed in
   `overlay/adapters-outside-overlay.txt`, and budgeted. Any of the three
   missing is a hard failure. A listed path that left the delta fails until
   you delete its line.
2. **Every `M` file is documented** in `TOUCHPOINTS.md` (heading containing
   the path in backticks).
3. **Per-file and total line budgets** in `overlay/delta-budget.tsv`, covering
   every `M` file and every sanctioned adapter. `changed-lines` is added +
   deleted from `git diff --numstat`. The budget only ratchets down.

```sh
overlay/scripts/overlay-diff.sh
overlay/scripts/overlay-diff.sh --update-budget
overlay/scripts/overlay-diff.sh --update-budget --allow-growth "why this growth is unavoidable"
```

`--update-budget` rewrites `delta-budget.tsv` from the current tree but
refuses to raise any number. If a file (or the total) grew, shrink the change
or re-run with `--allow-growth "<reason>"`, which writes the new budget and
appends a dated reason comment so growth leaves a permanent, reviewable trace.

`AGENTS.md` is excluded from the gates: it is fork policy at the repo root,
not an upstream touchpoint and not crates code.

Run the script before finishing any task that touched xAI's tree, and after
every sync. Install the pre-push hook once so pushes cannot skip it:

```sh
overlay/scripts/install-hooks.sh
```

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

## Definition of done

A change is finished when all of these hold:

- It sits at the lowest workable rung of the ladder.
- Any whole file we authored lives under `overlay/`, unless it is a sanctioned
  `overlay_*` adapter listed in `adapters-outside-overlay.txt` and budgeted.
- Any new upstream touchpoint (`M`) is recorded in `TOUCHPOINTS.md` and fits
  inside `overlay/delta-budget.tsv` (or the budget was deliberately updated
  with `--allow-growth` and a reason).
- `overlay/scripts/overlay-diff.sh` passes (Gates 1–3: adapters sanctioned,
  every `M` documented, line budget held).
- The pre-push hook is installed once on this clone via
  `overlay/scripts/install-hooks.sh` (idempotent; blocks pushes that fail the
  gates). Emergency bypass only: `git push --no-verify`.
- The overlay packages and the binary compile, and overlay tests pass.
- Our commits are separable and prefixed `overlay:`.
- `target/release/incremental` and `target/debug` are deleted, now that nothing
  else in the task needs them.
- The public repository and release discipline below is complete.

## Public repository and release discipline

- This fork and its `origin` repository must remain public.
- Never commit or release credentials, tokens, cookies, private keys, auth
  files, credentialed URLs, user data, or real machine-specific private paths.
  Before every push and release, scan `origin/main..HEAD`, tracked and
  untracked files, the final binary, archives, checksums, and release notes
  with Gitleaks plus targeted credential-pattern and local-path checks.
- A completed change is not done when it only works locally. Once the requested
  change is validated and accepted, commit it and push `main` without waiting
  for a separate user request.
- Create the next immutable `v<upstream-version>-overlay.<n>` tag and public
  GitHub Release only when the distributable Grok binary changes. Documentation,
  policy, and other source-only changes are pushed without compiling or
  releasing a new binary unless the user explicitly asks for one.
- Before an upstream sync or release, fetch `origin` and prove that the commit
  being published contains the current `origin/main`. Fast-forward stale local
  `main` refs or checkouts before running `sync-upstream.sh`; never let a stale
  worktree force-push over newer public commits.
- Build each release from the exact tagged commit with an explicit
  `GROK_VERSION`, remap local build paths, produce a stripped ad-hoc-signed
  macOS arm64 binary, and attach the direct binary, a license-complete archive,
  `SHA256SUMS`, release notes, the exact commit, and validation results.
- Verify that `origin/main`, the annotated tag, the GitHub Release, GitHub's
  uploaded asset digests, and the locally installed `~/bin/grok` binary all
  resolve to the same audited commit and checksums.
- Preserve every previous release and tag as a rollback point. Never move a
  published version tag or replace an existing release asset. A force-push is
  allowed only for the documented upstream-sync rebase flow and only with its
  required `pre-sync/*` rollback tag.
