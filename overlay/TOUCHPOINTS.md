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

- What: adds `overlay/overlay-core` and `overlay/overlay-subagent-router` to
  `[workspace] members`.
- Why here: cargo requires workspace members to be listed in the root manifest,
  and the header of that file says it is auto-generated upstream, so the line
  will keep disappearing on syncs.
- Retire when: never, while any overlay package exists. Expect a one-line
  conflict per sync; `git rerere` replays the resolution.

### `crates/common/xai-tool-types/src/task.rs`

- What: adds `task_type`, `complexity`, `requires_vision` on `TaskToolInput`,
  and rewrites the `model` schemars description as error-path override only.
- Why here: the parent-facing tool schema is defined in this shared crate;
  hooks cannot change JSON schema fields the model sees.
- Retire when: upstream adds first-class intent routing fields (unlikely).

### `crates/codegen/xai-grok-tools/Cargo.toml`

- What: path dependency on `overlay-subagent-router`.
- Why here: `TaskTool` lives in this package and must call the router at spawn.
- Retire when: never, while TaskTool applies overlay routing.

### `crates/codegen/xai-grok-tools/src/implementations/grok_build/task/mod.rs`

- What: `resolve_subagent_route` at the start of `TaskTool::run` applies router
  model, effort, unrestricted tool ceiling, and the provider-distinct fallback
  model list before validation and request build.
- Why here: this is the single model-facing spawn entry for the `task` tool;
  resolution in shell alone would miss inputs only present on the tool args.
- Retire when: upstream exposes a spawn-time routing hook with equivalent
  fields.

### `crates/codegen/xai-grok-shell/Cargo.toml`

- What: path dependency on `overlay-subagent-router`.
- Why here: the live child session owns retrying a failed model turn without
  creating a second subagent/session identity.
- Retire when: upstream exposes a provider-fallback hook inside child sampling.

### `crates/codegen/xai-grok-shell/src/agent/subagent/handle_request.rs`

- What: retry a provider-classified failed first turn on one representative
  model from each remaining configured provider, rewinding the failed prompt,
  switching the existing child session in place, and stopping at first success.
- Why here: this is the only boundary that sees the real provider sampling
  error while still owning the existing child identity and prompt lifecycle.
- Retire when: upstream supports provider-distinct failover for subagent turns.

### `crates/codegen/xai-grok-shell/src/agent/subagent/mod.rs`

- What: exposes the existing model-override resolver within the crate so the
  retry path can resolve each configured fallback model safely.
- Why here: the resolver depends on shell-private model/auth state.
- Retire when: same as the child retry touchpoint above.

### `crates/codegen/xai-grok-subagent-resolution/src/overrides.rs`

- What: initializes the new internal provider fallback list in a test helper.
- Why here: the helper constructs `SubagentRuntimeOverrides` explicitly instead
  of using `Default`.
- Retire when: the fallback field no longer lives on that shared runtime type.

### `crates/codegen/xai-grok-tools/src/implementations/grok_build/task/types.rs`

- What: adds the internal primary-provider label and provider-distinct fallback
  model list to `SubagentRuntimeOverrides`.
- Why here: this plain request type is the existing handoff from TaskTool routing
  to the shell child runner.
- Retire when: upstream exposes equivalent retry metadata.

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

### `crates/codegen/xai-grok-sampling-types/src/messages.rs`

- What: `#[serde(default)]` on `ContentBlock::Thinking::{thinking, signature}`.
- Why here: Anthropic opens a thinking block with
  `{"type":"thinking","thinking":""}` and fills it through deltas, and some
  routes never send plaintext at all — the thought comes back as a signed blob
  with no `thinking` field. Either shape aborted the stream before this. The
  `messages` backend is what gives Claude, GPT and Gemini a visible chain of
  thought through the local gateway, so the fork is unusable with reasoning
  models without it. Serde attributes have no config or overlay seam.
- Retire when: upstream tolerates thinking blocks that carry only a signature.

### `crates/codegen/xai-grok-pager-render/src/theme/iterm.rs`

- What: a new file holding the `iterm-green` theme, an iTerm2 profile (green
  `#76E765` on violet `#160C2A`) ported into the pager.
- Why here: themes are built by `Theme` constructors that the render crate
  dispatches internally; there is no registry an overlay package could add to.
- Retire when: upstream accepts user-defined themes from config, which would
  turn this into a `~/.grok` file and drop the delta to zero.

### `crates/codegen/xai-grok-pager-render/src/theme/codexdark.rs`

- What: a new file holding the `codex-dark` theme, ported verbatim from
  Conan Code's own embedded Ghostty theme
  (`Coinor/Resources/GhosttyOverrides.conf` in the `jattento/coinor` repo):
  background `#181818`, accent `#339CFF`, warm-cream foreground `#FAF3DD`,
  and its ANSI-16 diff/status ramp.
- Why here: same as `theme/iterm.rs` above.
- Retire when: same as above.

### `crates/codegen/xai-grok-pager-render/src/theme/mod.rs`

- What: registers both fork themes — `ThemeKind::ItermGreen = 6` and
  `ThemeKind::CodexDark = 7` — each with its `mod` declaration, display
  name, parse aliases, quantization flag, and the two dispatch arms mapping
  the kind to its `Theme::iterm_green()` / `Theme::codex_dark()`.
- Why here: `ThemeKind` is a closed enum with exhaustive matches, so a new
  theme cannot be added from outside the crate.
- Retire when: same as `theme/iterm.rs` above.

### `crates/codegen/xai-grok-pager-render/src/theme/cache.rs`

- What: one line per theme adding the new kind to the cached theme table.
- Why here: the cache enumerates every `ThemeKind`.
- Retire when: same as above.

### `crates/codegen/xai-grok-pager-render/src/syntax.rs`

- What: one line per theme adding the new kind to the syntax-highlighting
  match (both route to the GrokNight syntect theme — dark/neutral enough to
  share it).
- Why here: the match over `ThemeKind` is exhaustive.
- Retire when: same as above.

### `crates/codegen/xai-grok-pager/src/settings/defs.rs`

- What: adds `iterm-green` and `codex-dark` entries to `THEME_CHOICES` and
  `CONCRETE_THEME_CHOICES` so both themes are selectable from the Settings
  modal, not just `/theme` or the config file — the iterm-green commit added
  the theme but missed this catalog.
- Why here: these are hand-written catalogs, not derived from
  `ThemeKind::available()`.
- Retire when: same as `theme/iterm.rs` above.

### `crates/codegen/xai-grok-markdown` — table spans

Covers `crates/codegen/xai-grok-markdown/src/output.rs`,
`crates/codegen/xai-grok-markdown/src/render.rs`,
`crates/codegen/xai-grok-markdown/src/streaming.rs`,
`crates/codegen/xai-grok-markdown/src/buffers.rs`,
`crates/codegen/xai-grok-markdown/src/parse.rs` and
`crates/codegen/xai-grok-markdown/src/lib.rs`.

- What: adds a public `TableSpan { source, output_line_range, source_byte_range }`
  and a `tables` field on `MarkdownRenderOutput` / `MarkdownRenderView`, mirroring
  the pre-existing `CodeBlockSpan` / `code_blocks` exactly — same shape, same
  streaming rebase in `rerender_tail`. Plus a `TableReplace::is_table` flag,
  because that buffer is shared with display-math blocks and only real tables get
  a span.
- Why here: a rendered table is box-drawing art; its `|`-delimited source is
  consumed by the renderer and reaches no output type, so a copy affordance had
  nothing to put on the clipboard. `SourceMap` is ANSI-path only, so the byte
  range cannot be recovered downstream either. The span has to be emitted where
  `table_base_line` and `TableReplace::range` are both in hand — inside
  `render_ratatui`. No config, hook, or overlay seam exists for a new field on an
  upstream output struct.
- Retire when: upstream ships its own table spans (it already ships
  `CodeBlockSpan`, so this is the obvious next step and would let us delete the
  whole entry).

### `crates/codegen/xai-grok-pager/src/scrollback/blocks/mermaid_content.rs`

- What: generalizes the affordance row from Mermaid-only to every copyable
  markdown block. Adds `AffordanceSubject` (`Mermaid` / `Code(lang)` / `Table`),
  `AffordanceKind::Copy`, a `CopyBlock` record, and turns `affordance_row` into
  `affordance_row(subject, rendering)` with a `Vec<AffordanceButton>` and a
  `Cow` label. Mermaid's label, buttons, columns and status are unchanged and
  pinned by the existing tests.
- Why here: the row-insertion, layout and hit-rect machinery already lives in
  this file and is the single source of truth shared with the painter. Wrapping
  it from `overlay/` would mean duplicating that layout and reintroducing exactly
  the paint/hit-rect drift the module is written to prevent.
- Why the file keeps its Mermaid name: renaming it would rewrite every import
  and inflate the rebase surface for zero behavioural gain.
- Retire when: upstream adds copy affordances for code blocks and tables.

### `crates/codegen/xai-grok-pager` — copy-affordance producers

Covers `crates/codegen/xai-grok-pager/src/scrollback/blocks/markdown_content.rs`
and `crates/codegen/xai-grok-pager/src/scrollback/blocks/agent.rs`.

- What: `MarkdownContent::copy_blocks()` merges the view's code-block and table
  spans into one document-ordered list, `copy_block_counts()` counts them without
  allocating, and `AgentMessageBlock` drives affordance rows from that list
  instead of from Mermaid alone (caching the counts at construction/finish so the
  off-screen height estimate never rescans).
- Why here: these are the producers of the block's `output()` and of
  `diagram_affordances()`; the inserted rows and their anchored offsets must come
  from one layout pass, which only this code performs.
- Retire when: same as the entry above.

### `crates/codegen/xai-grok-pager` — copy-affordance painting and clicks

Covers `crates/codegen/xai-grok-pager/src/scrollback/render.rs`,
`crates/codegen/xai-grok-pager/src/app/agent_view/media.rs`,
`crates/codegen/xai-grok-pager/src/app/agent_view/paste.rs` and
`crates/codegen/xai-grok-pager/src/views/file_search/line_viewer.rs`.

- What: threads `AffordanceSubject` through `DiagramAffordancePlacement` into the
  painter, which now asks the row for its label/buttons per subject, skips the
  Mermaid render-state hash for code/table rows, and routes
  `AffordanceKind::Copy` to `copy_to_clipboard`. `paste.rs` is test-only fallout
  from the changed signatures.
  The line viewer is upstream's second producer of placements; it only ever
  emits Mermaid rows, so it names that subject explicitly.
- Why here: the placement struct and the painter are the only path from a
  rendered row to a click hit-rect, and every producer of the struct has to
  supply the new field.
- Retire when: same as the entries above.

### `crates/codegen/xai-grok-pager/src/diagnostics/doctor_format_tests.rs`

- What: derives the limited-color theme-count denominator from
  `ThemeKind::ALL` instead of hardcoding upstream's five themes.
- Why here: the fork's two extra themes legitimately change the denominator,
  while the available limited-color themes remain unchanged.
- Retire when: upstream makes this expectation dynamic.

### `crates/codegen/xai-grok-pager/src/doctor_cmd/tests.rs`

- What: derives human and JSON theme-count expectations from `ThemeKind::ALL`
  instead of hardcoding upstream's five themes.
- Why here: the fork's two extra themes legitimately change the denominator,
  and these contract fixtures must follow the enum used by their inputs.
- Retire when: upstream makes these expectations dynamic.

### `crates/codegen/xai-grok-pager/src/views/settings_modal/tests.rs`

- What: adds `ItermGreen` and `CodexDark` arms to an exhaustive match and uses
  their raw public `Theme` constructors for the palette contrast assertion.
- Why here: the fork variants require match arms, and `Theme::current()` is
  quantized so limited-color terminals can collapse the compared colors.
- Retire when: same as `theme/iterm.rs` above.
