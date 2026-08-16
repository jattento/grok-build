//! Entry point for fork-local behaviour.
//!
//! Upstream (`xai-org/grok-build`) is synced as wholesale snapshots, so every
//! line we add to `crates/` is a future merge conflict. All custom logic lives
//! here instead; the upstream tree only calls into it.
//!
//! See `AGENTS.md` at the repo root and `overlay/TOUCHPOINTS.md`.

pub mod themes;

/// Version of the overlay itself, independent from the upstream grok version.
pub const OVERLAY_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Env var that suppresses the startup banner when set to a falsy value.
const BANNER_ENV: &str = "GROK_OVERLAY_BANNER";

/// Every switch that keeps this build from reporting anything to xAI, and from
/// asking whether it may.
///
/// The values are upstream's own: `crates/codegen/xai-grok-test-support`
/// pins the same set for its hermetic sandbox, so these are the spellings the
/// resolvers are written and tested against, not guesses.
///
/// - `GROK_TELEMETRY_ENABLED` / `DISABLE_TELEMETRY` — product events.
/// - `GROK_TELEMETRY_MIXPANEL_ENABLED` — Mixpanel engagement events.
/// - `GROK_TELEMETRY_TRACE_UPLOAD` — turn traces uploaded to xAI's bucket.
/// - `DISABLE_ERROR_REPORTING` — Sentry.
/// - `GROK_INSTRUMENTATION` / `GROK_EXTERNAL_OTEL` /
///   `OTEL_SDK_DISABLED` — every OTLP exporter.
/// - `GROK_PRIVACY_NOTICE_ROLLOUT` — the coding-data consent banner. This is
///   the only one that is about the *question* rather than the data: the
///   banner is remote-cohort gated, and the env var is checked first.
///
/// Local `tracing` logging (`RUST_LOG`, `GROK_DEBUG_LOG`, the debug firehose)
/// is deliberately untouched. It never leaves the machine and turning it off
/// would only cost us diagnosability.
const SILENCED_REPORTING: [(&str, &str); 9] = [
    ("GROK_TELEMETRY_ENABLED", "false"),
    ("DISABLE_TELEMETRY", "1"),
    ("GROK_TELEMETRY_MIXPANEL_ENABLED", "false"),
    ("GROK_TELEMETRY_TRACE_UPLOAD", "false"),
    ("DISABLE_ERROR_REPORTING", "1"),
    ("GROK_INSTRUMENTATION", "disabled"),
    ("GROK_EXTERNAL_OTEL", "false"),
    ("OTEL_SDK_DISABLED", "true"),
    ("GROK_PRIVACY_NOTICE_ROLLOUT", "0"),
];

/// Called once from the binary's composition root, before anything else runs.
///
/// This is the single upstream touchpoint the overlay owns. Add new wiring
/// inside this function rather than adding new call sites in `crates/`.
pub fn install() {
    apply_reporting_silence();
    if banner_enabled(std::env::var(BANNER_ENV).ok().as_deref()) {
        eprintln!("[overlay {OVERLAY_VERSION}] active");
    }
}

/// Force the reporting switches into the process environment.
///
/// `install()` calls this before any other Grok code runs so env beats
/// config, remote flags, and later defaults. Existing values are overwritten:
/// a leftover `true` in the user's shell must not re-enable product reporting.
pub fn apply_reporting_silence() {
    for (key, value) in SILENCED_REPORTING {
        // SAFETY: `install()` is the binary composition root and runs from
        // `main` before any other threads exist. Tests call this on the
        // test thread they own.
        unsafe {
            std::env::set_var(key, value);
        }
    }
}

/// The silenced-reporting pairs, for tests and launch probes.
pub fn reporting_silence_pairs() -> &'static [(&'static str, &'static str)] {
    &SILENCED_REPORTING
}

/// Evidence-backed context window for one released model route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelWindowPolicy {
    pub route: &'static str,
    pub model: &'static str,
    pub context_window: u64,
    pub source: &'static str,
    pub source_url: &'static str,
}

pub const MODEL_WINDOWS_VERIFIED_ON: &str = "2026-08-15";
#[cfg(test)]
const MODEL_WINDOWS_VERIFIED_UNIX_DAYS: u64 = 20_680;
#[cfg(test)]
const MODEL_WINDOW_EVIDENCE_MAX_AGE_DAYS: u64 = 180;

const MODEL_WINDOWS: [ModelWindowPolicy; 23] = [
    ModelWindowPolicy {
        route: "builtin",
        model: "grok-4.6",
        context_window: 500_000,
        source: "xAI model catalog",
        source_url: "https://docs.x.ai/developers/models/grok-4-6",
    },
    ModelWindowPolicy {
        route: "builtin",
        model: "grok-4.5",
        context_window: 500_000,
        source: "xAI model catalog",
        source_url: "https://docs.x.ai/developers/models/grok-4.5",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "claude-fable-5",
        context_window: 1_000_000,
        source: "Anthropic model catalog",
        source_url: "https://docs.anthropic.com/en/docs/about-claude/models/overview",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "claude-opus-5",
        context_window: 1_000_000,
        source: "Anthropic model catalog",
        source_url: "https://docs.anthropic.com/en/docs/about-claude/models/overview",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "claude-sonnet-5",
        context_window: 1_000_000,
        source: "Anthropic model catalog",
        source_url: "https://docs.anthropic.com/en/docs/about-claude/models/overview",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "claude-haiku-4-5-20251001",
        context_window: 200_000,
        source: "Anthropic model catalog",
        source_url: "https://docs.anthropic.com/en/docs/about-claude/models/overview",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "gpt-5.6-sol",
        context_window: 372_000,
        source: "CLIProxyAPI 7.2.113 embedded Codex catalog (bc71c77)",
        source_url: "https://github.com/router-for-me/CLIProxyAPI",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "gpt-5.6-terra",
        context_window: 372_000,
        source: "CLIProxyAPI 7.2.113 embedded Codex catalog (bc71c77)",
        source_url: "https://github.com/router-for-me/CLIProxyAPI",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "gpt-5.6-luna",
        context_window: 372_000,
        source: "CLIProxyAPI 7.2.113 embedded Codex catalog (bc71c77)",
        source_url: "https://github.com/router-for-me/CLIProxyAPI",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "gemini-3.1-pro-preview",
        context_window: 1_048_576,
        source: "Google Gemini model catalog",
        source_url: "https://ai.google.dev/gemini-api/docs/models/gemini-3.1-pro-preview",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "gemini-3.1-flash-lite",
        context_window: 1_048_576,
        source: "Google Gemini model catalog",
        source_url: "https://ai.google.dev/gemini-api/docs/models/gemini-3.1-flash-lite",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "opencode-deepseek-v4-flash",
        context_window: 1_048_576,
        source: "DeepSeek V4 model catalog",
        source_url: "https://api-docs.deepseek.com/quick_start/pricing/",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "opencode-deepseek-v4-pro",
        context_window: 1_048_576,
        source: "DeepSeek V4 model catalog",
        source_url: "https://api-docs.deepseek.com/quick_start/pricing/",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "opencode-glm-5.1",
        context_window: 200_000,
        source: "Z.AI GLM-5.1 guide",
        source_url: "https://docs.z.ai/guides/llm/glm-5.1",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "opencode-glm-5.2",
        context_window: 1_000_000,
        source: "Z.AI GLM-5.2 guide",
        source_url: "https://docs.z.ai/guides/llm/glm-5.2",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "opencode-hy3",
        context_window: 262_144,
        source: "Tencent Hy3 model card",
        source_url: "https://github.com/Tencent-Hunyuan/Hy3",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "opencode-kimi-k3",
        context_window: 1_048_576,
        source: "Moonshot Kimi K3 guide",
        source_url: "https://platform.kimi.ai/docs/guide/kimi-k3-quickstart",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "opencode-mimo-v2.5",
        context_window: 1_000_000,
        source: "Xiaomi MiMo V2.5 model page",
        source_url: "https://mimo.xiaomi.com/mimo-v2-5/",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "opencode-minimax-m2.7",
        context_window: 204_800,
        source: "MiniMax text generation guide",
        source_url: "https://platform.minimax.io/docs/guides/text-generation",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "opencode-minimax-m3",
        context_window: 1_000_000,
        source: "MiniMax text generation guide",
        source_url: "https://platform.minimax.io/docs/guides/text-generation",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "opencode-qwen3.6-plus",
        context_window: 1_000_000,
        source: "Alibaba Cloud Model Studio catalog",
        source_url: "https://www.alibabacloud.com/help/en/model-studio/models",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "opencode-qwen3.7-max",
        context_window: 1_000_000,
        source: "Alibaba Cloud Model Studio catalog",
        source_url: "https://www.alibabacloud.com/help/en/model-studio/models",
    },
    ModelWindowPolicy {
        route: "cliproxy",
        model: "opencode-qwen3.7-plus",
        context_window: 1_000_000,
        source: "Alibaba Cloud Model Studio catalog",
        source_url: "https://www.alibabacloud.com/help/en/model-studio/models",
    },
];

/// Model limits shipped by this fork, including their evidence.
pub fn model_window_policies() -> &'static [ModelWindowPolicy] {
    &MODEL_WINDOWS
}

/// Apply a route-specific model limit without affecting an official API route
/// that happens to use the same model slug.
pub fn effective_context_window(
    route: Option<&str>,
    model: &str,
    configured: std::num::NonZeroU64,
) -> std::num::NonZeroU64 {
    let Some(policy) = MODEL_WINDOWS
        .iter()
        .find(|policy| Some(policy.route) == route && policy.model == model)
    else {
        return configured;
    };
    std::num::NonZeroU64::new(policy.context_window).expect("model window policies are non-zero")
}

/// Marker appended to the version badge on the welcome screen, so the running
/// build is identifiable from inside the TUI. The startup banner goes to
/// stderr and is wiped when the alternate screen takes over, which makes it
/// useless once the UI is up.
pub fn hero_suffix() -> String {
    format!("  · overlay {OVERLAY_VERSION}")
}

/// Banner is on by default; only the usual falsy spellings turn it off, which
/// matches how upstream reads its own on/off env vars.
fn banner_enabled(value: Option<&str>) -> bool {
    match value {
        None => true,
        Some(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "off" | "no"
        ),
    }
}

/// Text shown instead of running upstream's self-updater.
fn update_message() -> String {
    format!(
        "This is a custom build of Grok Build (overlay {OVERLAY_VERSION}), so \
         `grok update` is disabled.\n\n\
         Upstream's updater would download the official binary into ~/.grok and \
         relaunch into it, dropping every customization in this fork.\n\n\
         To update, sync the fork and rebuild:\n\n  \
         overlay/scripts/sync-upstream.sh\n  \
         grk --rebuild\n\n\
         See AGENTS.md for the full procedure."
    )
}

/// Called from the `update` subcommand. Explains how to update a fork build
/// and exits without touching the installed binaries.
pub fn block_update() -> ! {
    eprintln!("{}", update_message());
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::{banner_enabled, update_message};

    #[test]
    fn banner_defaults_on() {
        assert!(banner_enabled(None));
        assert!(banner_enabled(Some("1")));
        assert!(banner_enabled(Some("yes")));
    }

    #[test]
    fn falsy_values_disable_banner() {
        for v in ["0", "false", "OFF", " no ", ""] {
            assert!(!banner_enabled(Some(v)), "{v} should disable the banner");
        }
    }

    #[test]
    fn update_message_points_at_the_fork_workflow() {
        let msg = update_message();
        assert!(msg.contains("sync-upstream.sh"));
        assert!(msg.contains("grk --rebuild"));
    }

    #[test]
    fn silenced_reporting_covers_every_required_switch() {
        let pairs = super::reporting_silence_pairs();
        let map: std::collections::BTreeMap<_, _> = pairs.iter().copied().collect();
        assert_eq!(map.get("GROK_TELEMETRY_ENABLED"), Some(&"false"));
        assert_eq!(map.get("DISABLE_TELEMETRY"), Some(&"1"));
        assert_eq!(map.get("GROK_TELEMETRY_MIXPANEL_ENABLED"), Some(&"false"));
        assert_eq!(map.get("GROK_TELEMETRY_TRACE_UPLOAD"), Some(&"false"));
        assert_eq!(map.get("DISABLE_ERROR_REPORTING"), Some(&"1"));
        assert_eq!(map.get("GROK_INSTRUMENTATION"), Some(&"disabled"));
        assert_eq!(map.get("GROK_EXTERNAL_OTEL"), Some(&"false"));
        assert_eq!(map.get("OTEL_SDK_DISABLED"), Some(&"true"));
        assert_eq!(map.get("GROK_PRIVACY_NOTICE_ROLLOUT"), Some(&"0"));
        assert_eq!(pairs.len(), 9);
    }

    #[test]
    fn install_overrides_enabling_reporting_values_and_is_idempotent() {
        // Banner off so a later install() in this process stays quiet.
        unsafe {
            std::env::set_var(super::BANNER_ENV, "0");
        }
        for (key, _) in super::reporting_silence_pairs() {
            unsafe {
                std::env::set_var(key, "enabled-by-existing-environment");
            }
        }
        super::install();
        for (key, value) in super::reporting_silence_pairs() {
            assert_eq!(
                std::env::var(key).as_deref(),
                Ok(*value),
                "{key} must be forced to {value}"
            );
        }
        // A second apply is idempotent.
        super::apply_reporting_silence();
        for (key, value) in super::reporting_silence_pairs() {
            assert_eq!(std::env::var(key).as_deref(), Ok(*value));
        }
    }

    #[test]
    fn every_model_window_is_unique_cited_and_non_zero() {
        let mut keys = std::collections::BTreeSet::new();
        for policy in super::model_window_policies() {
            assert!(keys.insert((policy.route, policy.model)));
            assert!(policy.context_window > 0);
            assert!(!policy.source.is_empty());
            assert!(policy.source_url.starts_with("https://"));
        }
    }

    #[test]
    fn model_window_evidence_is_current() {
        let today = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock must be after Unix epoch")
            .as_secs()
            / 86_400;
        assert!(today >= super::MODEL_WINDOWS_VERIFIED_UNIX_DAYS);
        assert!(
            today
                <= super::MODEL_WINDOWS_VERIFIED_UNIX_DAYS
                    + super::MODEL_WINDOW_EVIDENCE_MAX_AGE_DAYS,
            "model-window evidence from {} is stale",
            super::MODEL_WINDOWS_VERIFIED_ON
        );
    }

    #[test]
    fn released_router_catalog_has_a_cited_window_for_every_model() {
        let starter = include_str!("../../subagent-router/subagent-router.starter.toml");
        for line in starter.lines().filter(|line| line.starts_with("[models.")) {
            let model = line
                .trim_start_matches("[models.")
                .trim_end_matches(']')
                .trim_matches('"');
            assert!(
                super::model_window_policies()
                    .iter()
                    .any(|policy| policy.model == model),
                "{model} needs a model-window policy"
            );
        }
    }

    #[test]
    fn route_policy_uses_practical_limits_without_clamping_official_apis() {
        use std::num::NonZeroU64;

        let configured = NonZeroU64::new(1_050_000).unwrap();
        assert_eq!(
            super::effective_context_window(Some("cliproxy"), "gpt-5.6-sol", configured).get(),
            372_000
        );
        assert_eq!(
            super::effective_context_window(Some("openai"), "gpt-5.6-sol", configured),
            configured
        );
        assert_eq!(
            super::effective_context_window(Some("cliproxy"), "opencode-glm-5.2", configured).get(),
            1_000_000
        );
    }
}
