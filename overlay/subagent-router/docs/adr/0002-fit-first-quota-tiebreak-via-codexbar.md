# Fit first; quota only ties; CodexBar CLI is the sensor

Among models listed for a route cell, only those that fit the cell are eligible. The sensor is `codexbar usage --format json` (shell-out); `dashboard` is not available on the installed CLI. Payloads expose `primary`/`secondary`/`tertiary`/`extraRateWindows` with `usedPercent` + `windowMinutes` — not named session/weekly/monthly. Windows are classified by duration thresholds. Any *present* window at/under the remaining floor vetoes the candidate (OpenCode’s ~30d tertiary is a real monthly gate). Among survivors, weekly-class remaining / time-to-reset ranks first; session-class breaks ties. Fetch only providers needed for the cell’s candidates; cache snapshots ~60–120s. Partial failures → macOS notification and drop those providers; if nothing usable remains → parent model.

**Status**: accepted

## Considered options

- Burn-down first (prefer expiring quota even with worse fit) — rejected: quality floor first.
- Live provider APIs or reading CodexBar cache files — rejected: CLI is the supported client contract.
- Hard-fail spawn when sensor or quota is empty — rejected: degrade to parent model.
- `codexbar guard` only — rejected: weekly/session only; misses monthly tertiary and multi-window veto.
- `codexbar serve` daemon — deferred; CLI + cache first.

## Consequences

- Every routable model slug needs an explicit provider binding in `~/.grok/subagent-router.toml`.
- Router depends on `codexbar` on PATH; full multi-provider probes can take ~1–2 minutes — must scope + cache.
- Codex-style `credits.remaining` is informational only in v1 — not an exhaustion veto (rate windows decide eligibility).
