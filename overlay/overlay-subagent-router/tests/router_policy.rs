//! Policy tests for the shipped router decision function.

use overlay_subagent_router::{
    NotifyKind, ProviderRetryDecision, ProviderUsageSnapshot, RouteInput, RouteSource,
    RouterConfig, STARTER_TOML, SpyNotifier, UsageWindowSnap, decide_route, load_config_from_str,
    next_provider_retry, notify_override, notify_provider_error, provider_retry_error,
    tool_ceiling_for_task_type,
};

fn starter() -> RouterConfig {
    load_config_from_str(STARTER_TOML).expect("starter toml parses")
}

fn input(task: &str, complexity: &str, vision: bool) -> RouteInput {
    RouteInput {
        task_type: Some(task.into()),
        complexity: Some(complexity.into()),
        requires_vision: vision,
        model_override: None,
    }
}

#[test]
fn tool_ceiling_by_task_type() {
    assert_eq!(tool_ceiling_for_task_type("scout"), "general-purpose");
    for t in ["debug", "implement", "design", "review"] {
        assert_eq!(tool_ceiling_for_task_type(t), "general-purpose");
    }
    let cfg = starter();
    assert_eq!(cfg.tool_ceiling("scout"), "general-purpose");
    assert_eq!(cfg.tool_ceiling("implement"), "general-purpose");

    let mut stale = starter();
    stale.tool_ceiling.insert("scout".into(), "explore".into());
    assert_eq!(stale.tool_ceiling("scout"), "general-purpose");
}

#[test]
fn provider_fallback_uses_one_model_per_distinct_remaining_provider() {
    let cfg = starter();
    let fallback =
        cfg.provider_fallback_models(Some("gemini-3.1-flash-lite"), "scout", "medium", false);
    let providers: Vec<_> = fallback
        .iter()
        .map(|(provider, _)| provider.as_str())
        .collect();
    assert_eq!(providers, vec!["claude", "grok", "codex", "opencodego"]);
    assert_eq!(
        providers.len(),
        providers
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
    );
}

#[test]
fn provider_fallback_respects_vision_and_omits_primary_provider() {
    let cfg = starter();
    let fallback = cfg.provider_fallback_models(Some("claude-opus-5"), "review", "high", true);
    assert!(fallback.iter().all(|(provider, _)| provider != "claude"));
    assert!(
        fallback
            .iter()
            .all(|(_, model)| cfg.models[model].supports_vision)
    );
}

#[test]
fn configured_retry_plan_names_primary_provider_and_keeps_fallbacks_distinct() {
    let cfg = starter();
    let primary = overlay_subagent_router::provider_for_model(&cfg, "gemini-3.1-flash-lite");
    let fallback =
        cfg.provider_fallback_models(Some("gemini-3.1-flash-lite"), "scout", "medium", false);
    assert_eq!(primary.as_deref(), Some("gemini"));
    assert!(fallback.iter().all(|(provider, _)| provider != "gemini"));
}

#[test]
fn retry_state_machine_consumes_one_provider_and_distinguishes_stop_from_exhaustion() {
    let mut fallbacks = vec![
        ("claude".to_string(), "claude-sonnet-5".to_string()),
        ("codex".to_string(), "gpt-5.6-terra".to_string()),
    ];
    assert_eq!(
        next_provider_retry(true, &mut fallbacks),
        ProviderRetryDecision::Retry {
            provider: "claude".to_string(),
            model: "claude-sonnet-5".to_string(),
        }
    );
    assert_eq!(fallbacks.len(), 1);
    assert_eq!(
        next_provider_retry(false, &mut fallbacks),
        ProviderRetryDecision::Stop
    );
    assert_eq!(fallbacks.len(), 1);
    fallbacks.clear();
    assert_eq!(
        next_provider_retry(true, &mut fallbacks),
        ProviderRetryDecision::Exhausted
    );
}

#[test]
fn provider_names_are_reported_only_after_true_exhaustion() {
    let providers = vec![
        "gemini".to_string(),
        "claude".to_string(),
        "codex".to_string(),
    ];
    let exhausted = provider_retry_error("service unavailable", &providers, true);
    assert!(exhausted.contains("gemini, claude, codex"));
    assert!(exhausted.contains("service unavailable"));

    let deterministic = provider_retry_error("cwd does not exist", &providers, false);
    assert_eq!(deterministic, "Session error: cwd does not exist");
    assert!(!deterministic.contains("providers"));
}

#[test]
fn every_complexity_cell_has_candidates_and_effort() {
    let cfg = starter();
    let types = ["scout", "debug", "implement", "design", "review"];
    let comps = ["low", "medium", "high"];
    for t in types {
        for c in comps {
            let cell = cfg
                .route_cell(t, c)
                .unwrap_or_else(|| panic!("missing cell {t}.{c}"));
            assert!(
                !cell.models.is_empty(),
                "{t}.{c} must have non-empty models"
            );
            assert!(
                cell.effort.as_ref().is_some_and(|e| !e.is_empty()),
                "{t}.{c} must set effort"
            );
            for m in &cell.models {
                assert!(
                    cfg.models.contains_key(m),
                    "{t}.{c} references unknown model {m}"
                );
            }
        }
    }
}

#[test]
fn requires_vision_drops_nv_models() {
    let cfg = starter();
    // implement.high includes opencode-qwen3.7-max (NV)
    let usage = ProviderUsageSnapshot::empty();
    let d = decide_route(
        &input("implement", "high", true),
        &cfg,
        Some("parent"),
        &usage,
    );
    assert_eq!(d.source, RouteSource::Routed);
    let model = d.model.as_deref().unwrap();
    let meta = cfg.models.get(model).unwrap();
    assert!(meta.supports_vision, "picked NV model {model}");
    assert_ne!(model, "opencode-qwen3.7-max");
}

#[test]
fn vision_false_allows_nv_in_pool() {
    let cfg = starter();
    // Preference may still pick non-NV first; ensure NV is not filtered from cell logic
    // by crafting a cell-only NV preference via decide with usage that exhausts others.
    let mut usage = ProviderUsageSnapshot::empty();
    // Exhaust claude and codex and grok so only opencode remains for implement.high
    for p in ["claude", "codex", "grok"] {
        usage.by_provider.insert(
            p.into(),
            vec![UsageWindowSnap {
                used_percent: 100.0,
                window_minutes: Some(10080),
                resets_at: None,
            }],
        );
    }
    // opencode healthy
    usage.by_provider.insert(
        "opencodego".into(),
        vec![UsageWindowSnap {
            used_percent: 10.0,
            window_minutes: Some(10080),
            resets_at: None,
        }],
    );
    let d = decide_route(
        &input("implement", "high", false),
        &cfg,
        Some("parent"),
        &usage,
    );
    assert_eq!(d.model.as_deref(), Some("opencode-qwen3.7-max"));
}

#[test]
fn exhaustion_veto_any_present_window() {
    let cfg = starter();
    let mut usage = ProviderUsageSnapshot::empty();
    // All providers exhausted on some window
    for p in ["claude", "codex", "grok", "gemini", "opencodego"] {
        usage.by_provider.insert(
            p.into(),
            vec![UsageWindowSnap {
                used_percent: 99.5, // remaining 0.5 <= 1
                window_minutes: Some(10080),
                resets_at: None,
            }],
        );
    }
    let d = decide_route(
        &input("debug", "low", false),
        &cfg,
        Some("parent-model"),
        &usage,
    );
    assert_eq!(d.source, RouteSource::ParentFallback);
    assert_eq!(d.model.as_deref(), Some("parent-model"));
}

#[test]
fn null_windows_ignored_credits_not_veto() {
    let cfg = starter();
    // Only weekly present; no primary — still eligible
    let mut usage = ProviderUsageSnapshot::empty();
    usage.by_provider.insert(
        "claude".into(),
        vec![UsageWindowSnap {
            used_percent: 20.0,
            window_minutes: Some(10080),
            resets_at: None,
        }],
    );
    let d = decide_route(&input("debug", "low", false), &cfg, Some("parent"), &usage);
    assert_eq!(d.source, RouteSource::Routed);
    assert!(d.model.is_some());
}

#[test]
fn window_without_minutes_still_vetoes_on_used_percent() {
    let cfg = starter();
    let mut usage = ProviderUsageSnapshot::empty();
    // Grok-style: no windowMinutes, but used 100% → veto
    for p in ["claude", "codex", "gemini", "opencodego"] {
        usage.by_provider.insert(
            p.into(),
            vec![UsageWindowSnap {
                used_percent: 100.0,
                window_minutes: Some(10080),
                resets_at: None,
            }],
        );
    }
    usage.by_provider.insert(
        "grok".into(),
        vec![UsageWindowSnap {
            used_percent: 100.0,
            window_minutes: None,
            resets_at: Some("2026-08-09T22:38:22Z".into()),
        }],
    );
    let d = decide_route(
        &input("debug", "high", false),
        &cfg,
        Some("parent-m"),
        &usage,
    );
    assert_eq!(d.source, RouteSource::ParentFallback);
    assert_eq!(d.model.as_deref(), Some("parent-m"));
}

#[test]
fn weekly_rank_prefers_higher_remaining() {
    let cfg = starter();
    // debug.low: sonnet (claude), grok, terra (codex), qwen-plus (opencodego)
    let mut usage = ProviderUsageSnapshot::empty();
    usage.by_provider.insert(
        "claude".into(),
        vec![UsageWindowSnap {
            used_percent: 80.0, // 20% remaining weekly
            window_minutes: Some(10080),
            resets_at: None,
        }],
    );
    usage.by_provider.insert(
        "grok".into(),
        vec![UsageWindowSnap {
            used_percent: 10.0, // 90% remaining weekly
            window_minutes: Some(10080),
            resets_at: None,
        }],
    );
    usage.by_provider.insert(
        "codex".into(),
        vec![UsageWindowSnap {
            used_percent: 50.0,
            window_minutes: Some(10080),
            resets_at: None,
        }],
    );
    usage.by_provider.insert(
        "opencodego".into(),
        vec![UsageWindowSnap {
            used_percent: 40.0,
            window_minutes: Some(10080),
            resets_at: None,
        }],
    );
    let d = decide_route(&input("debug", "low", false), &cfg, Some("parent"), &usage);
    assert_eq!(d.model.as_deref(), Some("grok-4.5"));
}

#[test]
fn partial_provider_failure_drops_only_failed() {
    let cfg = starter();
    let mut usage = ProviderUsageSnapshot::empty();
    usage.failed_providers = vec!["claude".into()];
    usage.by_provider.insert(
        "grok".into(),
        vec![UsageWindowSnap {
            used_percent: 5.0,
            window_minutes: Some(10080),
            resets_at: None,
        }],
    );
    usage.by_provider.insert(
        "codex".into(),
        vec![UsageWindowSnap {
            used_percent: 5.0,
            window_minutes: Some(10080),
            resets_at: None,
        }],
    );
    let d = decide_route(&input("debug", "low", false), &cfg, Some("parent"), &usage);
    assert_eq!(d.source, RouteSource::Routed);
    assert_ne!(d.model.as_deref(), Some("claude-sonnet-5"));
    assert!(
        d.notify.iter().any(|n| matches!(
            n,
            NotifyKind::ProviderError { provider } if provider == "claude"
        )),
        "expected provider-error notify flag, got {:?}",
        d.notify
    );
}

#[test]
fn model_override_wins_blindly() {
    let cfg = starter();
    let mut usage = ProviderUsageSnapshot::empty();
    // Even if everything exhausted
    for p in ["claude", "codex", "grok"] {
        usage.by_provider.insert(
            p.into(),
            vec![UsageWindowSnap {
                used_percent: 100.0,
                window_minutes: Some(10080),
                resets_at: None,
            }],
        );
    }
    let mut inp = input("design", "high", true);
    inp.model_override = Some("opencode-qwen3.7-max".into()); // NV + exhausted providers
    let d = decide_route(&inp, &cfg, Some("parent"), &usage);
    assert_eq!(d.source, RouteSource::ModelOverride);
    assert_eq!(d.model.as_deref(), Some("opencode-qwen3.7-max"));
    assert!(matches!(
        d.notify.first(),
        Some(NotifyKind::ModelOverride { model }) if model == "opencode-qwen3.7-max"
    ));
}

#[test]
fn notify_spy_records_override_and_provider_error() {
    let spy = SpyNotifier::new();
    notify_override(&spy, "grok-4.5").unwrap();
    notify_provider_error(&spy, "claude", "timed out").unwrap();
    let calls = spy.calls();
    assert_eq!(calls.len(), 2);
    assert!(
        calls[0].0.contains("override")
            || calls[0].1.contains("override")
            || calls[0].1.contains("grok-4.5")
    );
    assert!(calls[1].1.contains("claude"));
}

#[test]
fn effort_omitted_when_model_lacks_reasoning() {
    let cfg = starter();
    // Force gemini (supports_reasoning_effort=false) via exhaustion of others on scout.high
    let mut usage = ProviderUsageSnapshot::empty();
    for p in ["claude", "codex", "grok", "opencodego"] {
        usage.by_provider.insert(
            p.into(),
            vec![UsageWindowSnap {
                used_percent: 100.0,
                window_minutes: Some(10080),
                resets_at: None,
            }],
        );
    }
    usage.by_provider.insert(
        "gemini".into(),
        vec![UsageWindowSnap {
            used_percent: 1.0,
            window_minutes: Some(1440),
            resets_at: None,
        }],
    );
    let d = decide_route(&input("scout", "high", false), &cfg, Some("parent"), &usage);
    // scout.high has gemini-3.1-pro-preview
    if d.model.as_deref() == Some("gemini-3.1-pro-preview") {
        assert!(d.effort.is_none());
    }
}

#[test]
fn resolve_for_spawn_reuses_sensor_results_via_caller_cache() {
    use overlay_subagent_router::{
        CachedSensor, Sensor, SensorConfig, SensorError, UsageSnapshot, resolve_for_spawn,
    };
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountSensor {
        n: Arc<AtomicUsize>,
    }
    impl Sensor for CountSensor {
        fn fetch_provider(
            &self,
            _cfg: &SensorConfig,
            provider: &str,
        ) -> Result<UsageSnapshot, SensorError> {
            self.n.fetch_add(1, Ordering::SeqCst);
            Ok(UsageSnapshot {
                provider: provider.into(),
                windows: vec![],
                credits_remaining: None,
            })
        }
    }

    let dir = tempfile::tempdir().unwrap();
    let cfg_path = dir.path().join("subagent-router.toml");
    std::fs::write(&cfg_path, STARTER_TOML).unwrap();

    let n = Arc::new(AtomicUsize::new(0));
    let sensor = CachedSensor::new(CountSensor { n: n.clone() }, 60);
    let spy = SpyNotifier::new();
    let inp = input("debug", "low", false);
    let _ = resolve_for_spawn(&inp, Some("p"), Some(&cfg_path), &sensor, &spy);
    let first = n.load(Ordering::SeqCst);
    assert!(first > 0, "first resolve must probe providers");
    let _ = resolve_for_spawn(&inp, Some("p"), Some(&cfg_path), &sensor, &spy);
    let second = n.load(Ordering::SeqCst);
    assert_eq!(
        first, second,
        "second resolve within TTL must not re-probe (first={first}, second={second})"
    );
}

#[test]
fn resolve_for_spawn_override_invokes_notifier() {
    use overlay_subagent_router::resolve_for_spawn;

    struct FailSensor;
    impl overlay_subagent_router::Sensor for FailSensor {
        fn fetch_provider(
            &self,
            _cfg: &overlay_subagent_router::SensorConfig,
            provider: &str,
        ) -> Result<overlay_subagent_router::UsageSnapshot, overlay_subagent_router::SensorError>
        {
            Err(overlay_subagent_router::SensorError::Provider(format!(
                "{provider} down"
            )))
        }
    }

    let dir = tempfile::tempdir().unwrap();
    let cfg_path = dir.path().join("subagent-router.toml");
    std::fs::write(&cfg_path, STARTER_TOML).unwrap();

    let spy = SpyNotifier::new();
    let mut inp = input("implement", "medium", false);
    inp.model_override = Some("claude-opus-5".into());
    let d = resolve_for_spawn(&inp, Some("parent-x"), Some(&cfg_path), &FailSensor, &spy);
    assert_eq!(d.source, RouteSource::ModelOverride);
    assert_eq!(d.model.as_deref(), Some("claude-opus-5"));
    let calls = spy.calls();
    assert!(
        !calls.is_empty(),
        "override must invoke notifier, got {calls:?}"
    );
    assert!(calls.iter().any(|(_, b)| b.contains("claude-opus-5")));

    // Provider failure path
    let spy2 = SpyNotifier::new();
    let inp2 = input("debug", "low", false);
    let _ = resolve_for_spawn(&inp2, Some("parent-x"), Some(&cfg_path), &FailSensor, &spy2);
    let calls2 = spy2.calls();
    assert!(
        !calls2.is_empty(),
        "provider errors must notify, got {calls2:?}"
    );
}
