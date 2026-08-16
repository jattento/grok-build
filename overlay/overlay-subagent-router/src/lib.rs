//! Subagent router: parent sends intent; this crate picks model, effort, tools.
//!
//! Pure decision logic is side-effect free. CodexBar shell-out, cache, and
//! macOS notify live behind thin adapters so tests inject fixtures and spies.

mod auth_bridge;
mod config;
mod decision;
mod fallback;
mod notify;
mod provider_marker;
mod sensor;
mod windows;

pub use auth_bridge::{history_fallback_snapshot, parse_history_json};
pub use config::{
    DEFAULT_CONFIG_REL, ModelMeta, RouteCell, RouterConfig, STARTER_TOML, SensorConfig,
    VisionConfig, WindowsConfig, load_config, load_config_from_str, resolve_config_path,
    seed_config_if_missing,
};
pub use decision::{
    NotifyKind, ProviderUsageSnapshot, RouteDecision, RouteInput, RouteSource, UsageWindowSnap,
    WindowClass, decide_route, tool_ceiling_for_task_type,
};
pub use fallback::{
    ProviderRetryDecision, configured_provider_retry_plan, next_provider_retry, provider_for_model,
    provider_retry_error,
};
pub use provider_marker::{
    RETRYABLE_PROVIDER_FAILURE_KEY, apply_retryable_provider_failure_marker,
    is_retryable_provider_failure, merge_retryable_provider_failure,
    retryable_provider_failure_from_data,
};
pub use notify::{
    Notifier, OsascriptNotifier, SpyNotifier, notify_override, notify_provider_error,
};
pub use sensor::{
    CachedSensor, CodexBarSensor, MemoryCache, Sensor, SensorError, UsageSnapshot, fetch_providers,
    output_with_timeout, process_global_codexbar_sensor,
};
pub use windows::{classify_window, remaining_percent, window_exhausted};

/// High-level entry used at spawn time.
///
/// Loads config from disk (seeding starter if missing), fetches CodexBar usage
/// for candidate providers (unless override), decides route, and fires notifies.
pub fn resolve_for_spawn(
    input: &RouteInput,
    parent_model: Option<&str>,
    config_path: Option<&std::path::Path>,
    sensor: &dyn Sensor,
    notifier: &dyn Notifier,
) -> RouteDecision {
    let path = config_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(resolve_config_path);
    let _ = seed_config_if_missing(&path);
    let config = match load_config(&path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "subagent-router: config load failed; parent fallback");
            return RouteDecision {
                model: parent_model.map(|s| s.to_string()),
                effort: None,
                tool_ceiling: tool_ceiling_for_task_type(
                    input.task_type.as_deref().unwrap_or("implement"),
                )
                .to_string(),
                source: RouteSource::ParentFallback,
                notify: vec![],
                reason: format!("config load failed: {e}"),
            };
        }
    };

    // Error-path model override: blind honor + notify
    if let Some(m) = input
        .model_override
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        if config.override_cfg.enabled {
            let mut d = RouteDecision {
                model: Some(m.to_string()),
                effort: None,
                tool_ceiling: tool_ceiling_for_task_type(
                    input.task_type.as_deref().unwrap_or("implement"),
                )
                .to_string(),
                source: RouteSource::ModelOverride,
                notify: vec![NotifyKind::ModelOverride {
                    model: m.to_string(),
                }],
                reason: "error-path model override".into(),
            };
            if config.override_cfg.notify_on_use {
                for n in &d.notify {
                    if let Err(e) = fire_notify(notifier, n) {
                        tracing::error!(error = %e, "subagent-router override notification failed");
                    }
                }
            }
            // Still derive tool ceiling from task_type when present
            if let Some(tt) = input.task_type.as_deref() {
                d.tool_ceiling = tool_ceiling_for_task_type(tt).to_string();
            }
            return d;
        }
    }

    let task_type = match input.task_type.as_deref() {
        Some(t) if !t.trim().is_empty() => t.trim(),
        _ => {
            return RouteDecision {
                model: parent_model.map(|s| s.to_string()),
                effort: None,
                tool_ceiling: "general-purpose".into(),
                source: RouteSource::ParentFallback,
                notify: vec![],
                reason: "missing task_type; parent fallback".into(),
            };
        }
    };
    let complexity = match input.complexity.as_deref() {
        Some(c) if !c.trim().is_empty() => c.trim(),
        _ => {
            return RouteDecision {
                model: parent_model.map(|s| s.to_string()),
                effort: None,
                tool_ceiling: tool_ceiling_for_task_type(task_type).to_string(),
                source: RouteSource::ParentFallback,
                notify: vec![],
                reason: "missing complexity; parent fallback".into(),
            };
        }
    };

    // Collect candidate providers for scoped fetch
    let cell = config.route_cell(task_type, complexity);
    let providers: Vec<String> = cell
        .map(|c| {
            c.models
                .iter()
                .filter_map(|m| config.models.get(m).map(|meta| meta.provider.clone()))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect()
        })
        .unwrap_or_default();

    let (snapshots, failed) = if providers.is_empty() {
        (vec![], vec![])
    } else {
        fetch_providers(sensor, &config.sensor, &providers)
    };

    let usage = ProviderUsageSnapshot::from_fetch(&snapshots, &failed);
    // `decide_route` already attaches ProviderError notifies for `failed`.
    // Do not re-extend here or macOS gets a double banner + double Glass sound.
    let decision = decide_route(input, &config, parent_model, &usage);

    for n in &decision.notify {
        if matches!(n, NotifyKind::ProviderError { .. }) && !config.sensor.notify_on_provider_error
        {
            continue;
        }
        if let Err(e) = fire_notify(notifier, n) {
            tracing::error!(error = %e, "subagent-router notification failed");
        }
    }

    decision
}

fn fire_notify(notifier: &dyn Notifier, kind: &NotifyKind) -> std::io::Result<()> {
    match kind {
        NotifyKind::ModelOverride { model } => notify_override(notifier, model),
        NotifyKind::ProviderError { provider } => {
            notify_provider_error(notifier, provider, "usage fetch failed (live + history)")
        }
    }
}
