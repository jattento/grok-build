//! Pure route decision: fit → vision → veto/rank → fallback; override is separate.

use crate::config::{RouterConfig, WindowsConfig};
use crate::windows::{classify_window, remaining_percent, window_exhausted};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowClass {
    Session,
    Weekly,
    Monthly,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteSource {
    Routed,
    ModelOverride,
    ParentFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotifyKind {
    ModelOverride { model: String },
    ProviderError { provider: String },
}

#[derive(Debug, Clone)]
pub struct RouteInput {
    pub task_type: Option<String>,
    pub complexity: Option<String>,
    pub requires_vision: bool,
    /// Error-path model override (when set, decide_route is not used for model).
    pub model_override: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RouteDecision {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub tool_ceiling: String,
    pub source: RouteSource,
    pub notify: Vec<NotifyKind>,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct UsageWindowSnap {
    pub used_percent: f64,
    pub window_minutes: Option<u64>,
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderUsageSnapshot {
    /// provider id → windows (all present windows)
    pub by_provider: std::collections::HashMap<String, Vec<UsageWindowSnap>>,
    pub failed_providers: Vec<String>,
}

impl ProviderUsageSnapshot {
    pub fn from_fetch(snapshots: &[crate::sensor::UsageSnapshot], failed: &[String]) -> Self {
        let mut by_provider = std::collections::HashMap::new();
        for s in snapshots {
            by_provider.insert(s.provider.clone(), s.windows.clone());
        }
        Self {
            by_provider,
            failed_providers: failed.to_vec(),
        }
    }

    pub fn empty() -> Self {
        Self::default()
    }
}

/// Constant tool ceiling for every child task type.
pub fn tool_ceiling_for_task_type(_task_type: &str) -> &'static str {
    "general-purpose"
}

/// Pure decision given config + usage snapshot (no I/O).
pub fn decide_route(
    input: &RouteInput,
    config: &RouterConfig,
    parent_model: Option<&str>,
    usage: &ProviderUsageSnapshot,
) -> RouteDecision {
    let task_type = input
        .task_type
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("implement");
    let complexity = input
        .complexity
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("medium");

    let tool_ceiling = config.tool_ceiling(task_type).to_string();
    let requires_vision = input.requires_vision || config.vision.default_requires_vision;

    // Error-path override handled by caller usually; support here too for pure tests.
    // Always honored (no `enabled` gate — that field was removed; notify is separate).
    if let Some(m) = input
        .model_override
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return RouteDecision {
            model: Some(m.to_string()),
            effort: None,
            tool_ceiling,
            source: RouteSource::ModelOverride,
            notify: vec![NotifyKind::ModelOverride {
                model: m.to_string(),
            }],
            reason: "error-path model override".into(),
        };
    }

    let Some(cell) = config.route_cell(task_type, complexity) else {
        return parent_fallback(
            parent_model,
            tool_ceiling,
            format!("no route cell for {task_type}.{complexity}"),
            usage,
        );
    };

    // Fit list in preference order
    let mut candidates: Vec<String> = cell.models.clone();
    let cell_effort = cell.effort.clone();

    // Vision filter
    if requires_vision {
        candidates.retain(|m| {
            config
                .models
                .get(m)
                .map(|meta| meta.supports_vision)
                .unwrap_or(true)
        });
        if candidates.is_empty() {
            return parent_fallback(
                parent_model,
                tool_ceiling,
                "no vision-capable candidates".into(),
                usage,
            );
        }
    }

    // Drop models whose providers failed entirely? No — only if we have no data
    // and cannot assess. Policy: if provider failed, drop those candidates
    // (cannot confirm quota); notify already recorded by caller.
    let failed: std::collections::HashSet<&str> =
        usage.failed_providers.iter().map(|s| s.as_str()).collect();

    let mut eligible: Vec<RankedCandidate> = Vec::new();
    for (pref_idx, model) in candidates.iter().enumerate() {
        let Some(meta) = config.models.get(model) else {
            continue;
        };
        if failed.contains(meta.provider.as_str()) {
            continue;
        }
        // If we have usage for this provider, apply veto; if no data at all for
        // provider (not failed, just missing), keep candidate (preference order).
        if let Some(windows) = usage.by_provider.get(&meta.provider) {
            if provider_exhausted(windows, &config.windows) {
                continue;
            }
            eligible.push(RankedCandidate {
                model: model.clone(),
                pref_idx,
                weekly_remaining: best_remaining(windows, WindowClass::Weekly, &config.windows),
                weekly_reset: best_reset_secs(windows, WindowClass::Weekly, &config.windows),
                session_remaining: best_remaining(windows, WindowClass::Session, &config.windows),
                supports_effort: meta.supports_reasoning_effort,
            });
        } else if usage.by_provider.is_empty() && failed.is_empty() {
            // No sensor data at all — keep preference order
            eligible.push(RankedCandidate {
                model: model.clone(),
                pref_idx,
                weekly_remaining: None,
                weekly_reset: None,
                session_remaining: None,
                supports_effort: meta.supports_reasoning_effort,
            });
        } else {
            // Other providers have data but this one missing — still eligible
            // without rank signals (preference only)
            eligible.push(RankedCandidate {
                model: model.clone(),
                pref_idx,
                weekly_remaining: None,
                weekly_reset: None,
                session_remaining: None,
                supports_effort: meta.supports_reasoning_effort,
            });
        }
    }

    if eligible.is_empty() {
        return parent_fallback(
            parent_model,
            tool_ceiling,
            "no eligible candidates after vision/quota".into(),
            usage,
        );
    }

    eligible.sort_by(|a, b| {
        // Higher weekly remaining first
        cmp_opt_f64_desc(a.weekly_remaining, b.weekly_remaining)
            // Sooner weekly reset (burn what renews soon) — lower secs first
            .then_with(|| cmp_opt_f64_asc(a.weekly_reset, b.weekly_reset))
            // Higher session remaining
            .then_with(|| cmp_opt_f64_desc(a.session_remaining, b.session_remaining))
            // Original preference
            .then_with(|| a.pref_idx.cmp(&b.pref_idx))
    });

    let winner = &eligible[0];
    let effort = if winner.supports_effort {
        cell_effort.clone()
    } else {
        None
    };

    RouteDecision {
        model: Some(winner.model.clone()),
        effort,
        tool_ceiling,
        source: RouteSource::Routed,
        notify: usage
            .failed_providers
            .iter()
            .map(|p| NotifyKind::ProviderError {
                provider: p.clone(),
            })
            .collect(),
        reason: format!(
            "routed {task_type}.{complexity} → {} (pref_idx={})",
            winner.model, winner.pref_idx
        ),
    }
}

struct RankedCandidate {
    model: String,
    pref_idx: usize,
    weekly_remaining: Option<f64>,
    weekly_reset: Option<f64>,
    session_remaining: Option<f64>,
    supports_effort: bool,
}

fn provider_exhausted(windows: &[UsageWindowSnap], cfg: &WindowsConfig) -> bool {
    for w in windows {
        if window_exhausted(w.used_percent, cfg.min_remaining_percent) {
            return true;
        }
    }
    false
}

fn best_remaining(
    windows: &[UsageWindowSnap],
    class: WindowClass,
    cfg: &WindowsConfig,
) -> Option<f64> {
    windows
        .iter()
        .filter(|w| classify_window(w.window_minutes, cfg) == class)
        .map(|w| remaining_percent(w.used_percent))
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
}

fn best_reset_secs(
    windows: &[UsageWindowSnap],
    class: WindowClass,
    cfg: &WindowsConfig,
) -> Option<f64> {
    // Without parsing timestamps in pure tests, use window_minutes as proxy
    // for "sooner reset" when resets_at is absent; prefer smaller minutes
    // among same class when ranking burn-down is close.
    windows
        .iter()
        .filter(|w| classify_window(w.window_minutes, cfg) == class)
        .filter_map(|w| w.window_minutes.map(|m| m as f64))
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
}

fn cmp_opt_f64_desc(a: Option<f64>, b: Option<f64>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(x), Some(y)) => y.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None) => std::cmp::Ordering::Less, // prefer known
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn cmp_opt_f64_asc(a: Option<f64>, b: Option<f64>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn parent_fallback(
    parent_model: Option<&str>,
    tool_ceiling: String,
    reason: String,
    usage: &ProviderUsageSnapshot,
) -> RouteDecision {
    RouteDecision {
        model: parent_model.map(|s| s.to_string()),
        effort: None,
        tool_ceiling,
        source: RouteSource::ParentFallback,
        notify: usage
            .failed_providers
            .iter()
            .map(|p| NotifyKind::ProviderError {
                provider: p.clone(),
            })
            .collect(),
        reason,
    }
}
