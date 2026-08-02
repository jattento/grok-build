# Touchpoints

Every file outside `overlay/` that differs from `upstream/main`, and why.
This file is the merge-cost ledger of the fork: each entry is a place that can
conflict on a sync. Keep it short and keep it honest.

Regenerate the real list with `overlay/scripts/overlay-diff.sh`; it fails if a
changed upstream file has no entry here.

Template for new entries:

```
### `path/to/file`

- What: one line describing the edit.
- Why here: why it could not be done from overlay/, hooks, or config.
- Retire when: the upstream change that would let us delete this.
```

---

### `Cargo.toml`

- What: adds `overlay/overlay-core` to `[workspace] members`.
- Why here: cargo requires workspace members to be listed in the root manifest,
  and the header of that file says it is auto-generated upstream, so the line
  will keep disappearing on syncs.
- Retire when: never, while any overlay package exists. Expect a one-line
  conflict per sync; `git rerere` replays the resolution.

### `crates/codegen/xai-grok-pager-bin/Cargo.toml`

- What: adds the `overlay-core` path dependency.
- Why here: the dependency must be declared by the package that links it.
- Retire when: never, while the binary calls into the overlay.

### `crates/codegen/xai-grok-pager-bin/src/main.rs`

- What: two single lines. `overlay_core::install();` right after
  `xai_grok_pager_minimal::install();` in `fn main()`, and
  `overlay_core::block_update();` as the first statement of the
  `Command::Update` arm.
- Why here: `fn main()` is the composition root — the only place that runs
  once, before any mode (TUI, headless, ACP, leader) is selected. Upstream
  already installs its own hooks in the same spot, so our line sits among
  similar neighbours and rarely lands in a conflicting hunk.
- Why a second call site: upstream's updater downloads the official binary into
  `~/.grok` and `restart_grok()` then prefers `~/.grok/bin/grok` over the
  running executable, so an update silently replaces this fork mid-session.
  That has to be intercepted at the `update` subcommand itself; it cannot be
  expressed from inside `install()`. It also covers the auto-update path, which
  works by spawning `<self> update`.
- Caveat: `--version` and `doctor` return before `install()`, so the overlay is
  not active for those two fast paths.
- Retire when: never. This is the intended single entry point; new behaviour
  goes inside `overlay_core::install()`, not into new call sites here.

### `crates/codegen/xai-grok-pager/Cargo.toml`

- What: adds the `overlay-core` path dependency.
- Why here: the dependency must be declared by the package that links it, and
  the welcome view lives in this package.
- Retire when: the welcome-screen marker below is dropped.

### `crates/codegen/xai-grok-pager/src/views/welcome/mod.rs`

- What: one `spans.push(overlay_core::hero_suffix())` in the version badge,
  placed after the `VersionBadgeMode` match so it covers every layout.
- Why here: the startup banner goes to stderr and is erased the moment the
  alternate screen takes over, so it is invisible in the TUI. The version badge
  is the only element rendered in all welcome layouts.
- Why after the match and not inside an arm: the badge has several modes and
  the terminal width decides which one runs; a per-arm edit would show the
  marker only at some widths and would mean several touchpoints instead of one.
- Retire when: we no longer want the running build identified on screen.

### `Cargo.lock`

- What: the lockfile entry for `overlay-core` (and anything it ever depends on).
- Why here: cargo regenerates the shared lockfile as soon as the workspace gains
  a member; this is a side effect, not a decision.
- Retire when: never. On conflict, take upstream's file wholesale and re-run
  `cargo check -p xai-grok-pager-bin` to re-add our entries. Keeping
  `overlay-*` packages dependency-free keeps this diff to a few lines.

### `crates/codegen/xai-grok-sampling-types/src/types.rs`

- What: `#[serde(default)]` on the streaming chat-completions types that the
  wire may legitimately omit: `ChatCompletionChunk::{id, object, created,
  model, choices, usage}`, `Usage::{prompt_tokens, completion_tokens,
  total_tokens}`, `ToolCallDelta::{id, kind, function}`, and
  `ChatChunkDelta::{role, content, reasoning_content, tool_call_id}`. No field
  was removed, retyped, or reordered.
- Why here: these structs are deserialized straight from the SSE stream by
  `xai-grok-sampler`, and a single chunk missing any of these fields kills the
  whole turn with `serialization error: missing field ...`. Against the local
  CLIProxyAPI gateway (the `cliproxy` models in `~/.grok/config.toml`) this made
  most non-xAI routes unusable: Gemini sends interim `usage` objects without
  `total_tokens`, OpenCode sends tool-call argument deltas without `id`/`type`
  and a trailing `{"choices":[],"cost":"0"}` chunk with no envelope fields.
  `ToolCallDelta`'s own doc comment already promises every field but `index` is
  optional, so this is upstream intent that serde was not told about. Serde
  attributes have no config, hook, or overlay seam.
- Retire when: upstream makes the stream types tolerant of partial chunks.
