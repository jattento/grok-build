# Parent sends intent; router resolves runtime

The parent-facing spawn contract exposes `task_type` and `complexity` (plus prompt/description and operational fields like isolation/cwd/resume), not `subagent_type`. The subagent router chooses model, reasoning effort, and tool ceiling by default. An optional `model` field exists only as an **error-path override**: tool/schema text must tell the parent to set it solely when a previous child for that work failed; routine spawns leave it unset. Every use of the override triggers a macOS notification so recovery choices are audible/visible.

**Status**: accepted

## Considered options

- Parent still chooses `subagent_type`; router only model/effort — rejected: user wants the parent free of tool/type choice; scout alone maps to explore.
- No model field at all — rejected: need a recovery hatch when the routed model fails the task.
- Unconstrained model override anytime — rejected: reintroduces the bad habit the router exists to stop; policy is error-path only (enforced primarily by instruction text + notification, not a hard session graph unless we add that later).
- Separate `task_routed` tool beside classic `task` — rejected: two paths confuse the parent.

## Consequences

- Plan-as-subagent is out of scope: planning stays on the main agent.
- Every routed child uses the unrestricted `general-purpose` runtime. `scout`
  remains a cognitive intent and model/effort route, not a reduced toolset.
- Upstream `TaskToolInput` shape (or an overlay-shaped view of it) must change or be adapted at the spawn boundary.
- Override policy is soft (tool description) unless we later gate on prior failed `task` ids; when `model` is present it wins over the router (no fit/quota veto) and a macOS notification is mandatory.
