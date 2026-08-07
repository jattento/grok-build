# Subagent Router

Domain for choosing which model, reasoning effort, and tool ceiling a child agent gets when the parent delegates work. The parent declares *intent*; the router resolves *runtime*.

## Language

**Task type**:
The kind of cognitive work the parent is delegating. Closed set: `scout`, `debug`, `implement`, `design`, `review`.
_Avoid_: subagent type, role, persona, agent type (those are different axes)

**Complexity**:
How hard that piece of work is. Closed set: `low`, `medium`, `high`. Drives both model ceiling and reasoning effort within a task type.
_Avoid_: priority, urgency, size (not the same as difficulty)

**Subagent router**:
The component that turns task type + complexity (+ live quota) into model, effort, and derived tool ceiling for a child spawn.
_Avoid_: model picker, load balancer, orchestrator

**Fit**:
Whether a model is an allowed candidate for a given task type × complexity cell. Fit is evaluated before quota.
_Avoid_: preference, score (until ranked among fit candidates)

**Quota tiebreak**:
Using remaining subscription capacity (and time-to-reset) only to choose among models that already fit. Never promotes a non-fit model.
_Avoid_: cost optimization, least-loaded (unless defined as this)

**Usage window**:
A CodexBar capacity interval on a provider payload: `primary`, `secondary`, `tertiary`, or an `extraRateWindows[]` entry. Each has `usedPercent`, optional `windowMinutes` and `resetsAt`. Absent/null windows are ignored, not treated as free or exhausted.
_Avoid_: assuming fields named session/weekly/monthly; those labels are derived

**Window class**:
Label derived from `windowMinutes` via configurable thresholds (defaults: short/session ≲6h, mid/weekly ~1–8d, long/monthly ≳28d). Used for ranking priority and human config, not raw API field names.
_Avoid_: hard-coding primary=session (Codex often has primary null and weekly on secondary)

**Weekly window**:
A usage window classified as mid/weekly when present. Primary ranking signal among eligible candidates.
_Avoid_: treating secondary always as weekly

**Session window**:
A usage window classified as short/session. Secondary fine-break when weekly is close.
_Avoid_: Grok session / chat session

**Exhaustion veto**:
If any *present* usage window (including extra rate windows) has remaining at or below the configured floor, that candidate is ineligible. Covers OpenCode’s ~30d tertiary monthly cap.
_Avoid_: ranking a provider dead on one window; treating null windows as vetoes

**Quota snapshot cache**:
Short-lived local cache of CodexBar usage JSON (TTL on the order of 1–2 minutes) so spawn does not wait on a full multi-provider probe (~tens of seconds).
_Avoid_: fetching `--provider all` on every spawn

**Credits**:
Optional prepaid/bonus balance on some providers (e.g. Codex `credits.remaining`). Informational for v1 — not used as an exhaustion veto.
_Avoid_: treating credits=0 as “provider dead”

**Error-path model override**:
Optional `model` on the parent-facing spawn tool, allowed only as a recovery path when a prior child for the same work failed (errors / unusable result) — not for routine routing. When set, it is honored blindly (no fit/quota/vision veto); a macOS notification is mandatory. Policy is taught in the tool description; normal spawns omit `model` and the router owns the choice.
_Avoid_: unconstrained routine model picking; silent overrides; applying exhaustion veto to recovery overrides

**Requires vision**:
Boolean spawn argument: the child must understand images (screenshots, UI captures, attached pictures). When true, candidates with `supports_vision = false` (NV / no-vision models) are removed before quota ranking.
_Avoid_: assuming every model can see; inferring vision only from the provider name

**NV model**:
A catalog model marked without vision (`supports_vision = false`), often shown as “(NV)” in the Grok display name. Eligible only when `requires_vision` is false.
_Avoid_: treating NV as a provider or task type

**Parent model fallback**:
When no fitted candidate has usable quota data (or every sensor path fails), use the model of the parent session and continue the spawn.
_Avoid_: hard fail, silent default to a fixed third model (unless config later says so)

**Tool ceiling**:
The maximum tools the child may use, derived from task type: `scout` → explore (no edits); every other task type → general-purpose. Plan work stays on the parent; it is not a child task type.
_Avoid_: capability_mode as the parent-facing input (parent does not pick this)

**Provider binding**:
Explicit map from a Grok model slug to a CodexBar provider id used for quota reads (e.g. `claude-opus-5` → `claude`).
_Avoid_: inferring provider from cliproxy (cliproxy is the transport, not the subscription)

**Route cell**:
One entry in the router config for a task type × complexity pair: candidate models (ordered preference before quota) and effort.
_Avoid_: global model list without cells
