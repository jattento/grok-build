//! End-to-end routing against resolve_for_spawn (real entry point).

use overlay_subagent_router::{
    RouteInput, RouteSource, STARTER_TOML, Sensor, SensorConfig, SensorError, SpyNotifier,
    UsageSnapshot, UsageWindowSnap, resolve_for_spawn,
};

struct FixtureSensor {
    snaps: std::collections::HashMap<String, UsageSnapshot>,
    fail: std::collections::HashSet<String>,
}

impl Sensor for FixtureSensor {
    fn fetch_provider(
        &self,
        _cfg: &SensorConfig,
        provider: &str,
    ) -> Result<UsageSnapshot, SensorError> {
        if self.fail.contains(provider) {
            return Err(SensorError::Provider(format!("{provider} timeout")));
        }
        // Missing ≠ failed: return empty windows so preference order applies
        // without treating the provider as a hard sensor error.
        Ok(self.snaps.get(provider).cloned().unwrap_or(UsageSnapshot {
            provider: provider.into(),
            windows: vec![],
            credits_remaining: None,
        }))
    }
}

fn weekly(used: f64) -> UsageWindowSnap {
    UsageWindowSnap {
        used_percent: used,
        window_minutes: Some(10080),
        resets_at: None,
    }
}

#[test]
fn e2e_normal_route_task_type_complexity() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("subagent-router.toml");
    std::fs::write(&path, STARTER_TOML).unwrap();

    let mut snaps = std::collections::HashMap::new();
    snaps.insert(
        "claude".into(),
        UsageSnapshot {
            provider: "claude".into(),
            windows: vec![weekly(10.0)],
            credits_remaining: None,
        },
    );
    snaps.insert(
        "codex".into(),
        UsageSnapshot {
            provider: "codex".into(),
            windows: vec![weekly(50.0)],
            credits_remaining: Some(0.0),
        },
    );
    snaps.insert(
        "grok".into(),
        UsageSnapshot {
            provider: "grok".into(),
            windows: vec![weekly(20.0)],
            credits_remaining: None,
        },
    );
    snaps.insert(
        "opencodego".into(),
        UsageSnapshot {
            provider: "opencodego".into(),
            windows: vec![weekly(30.0)],
            credits_remaining: None,
        },
    );

    let sensor = FixtureSensor {
        snaps,
        fail: Default::default(),
    };
    let spy = SpyNotifier::new();
    let input = RouteInput {
        task_type: Some("implement".into()),
        complexity: Some("medium".into()),
        requires_vision: false,
        model_override: None,
    };
    let d = resolve_for_spawn(&input, Some("parent-m"), Some(&path), &sensor, &spy);
    assert_eq!(d.source, RouteSource::Routed, "reason={}", d.reason);
    assert!(d.model.is_some(), "expected model, got {:?}", d);
    assert_eq!(d.tool_ceiling, "general-purpose");
    assert!(d.effort.as_deref().is_some_and(|e| !e.is_empty()));
    println!(
        "normal_route model={} effort={:?} ceiling={} reason={}",
        d.model.as_deref().unwrap(),
        d.effort,
        d.tool_ceiling,
        d.reason
    );
}

#[test]
fn e2e_requires_vision_filters_nv() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("subagent-router.toml");
    std::fs::write(&path, STARTER_TOML).unwrap();

    // Exhaust non-NV first prefs so NV would win without filter on implement.high
    let mut snaps = std::collections::HashMap::new();
    for (p, used) in [
        ("claude", 100.0),
        ("codex", 100.0),
        ("grok", 100.0),
        ("opencodego", 5.0),
    ] {
        snaps.insert(
            p.into(),
            UsageSnapshot {
                provider: p.into(),
                windows: vec![weekly(used)],
                credits_remaining: None,
            },
        );
    }
    let sensor = FixtureSensor {
        snaps,
        fail: Default::default(),
    };
    let spy = SpyNotifier::new();

    let without = resolve_for_spawn(
        &RouteInput {
            task_type: Some("implement".into()),
            complexity: Some("high".into()),
            requires_vision: false,
            model_override: None,
        },
        Some("parent"),
        Some(&path),
        &sensor,
        &spy,
    );
    assert_eq!(without.model.as_deref(), Some("opencode-qwen3.7-max"));

    let with = resolve_for_spawn(
        &RouteInput {
            task_type: Some("implement".into()),
            complexity: Some("high".into()),
            requires_vision: true,
            model_override: None,
        },
        Some("parent-vision"),
        Some(&path),
        &sensor,
        &spy,
    );
    // All vision-capable candidates exhausted → parent fallback
    assert_eq!(with.source, RouteSource::ParentFallback);
    assert_eq!(with.model.as_deref(), Some("parent-vision"));
    println!(
        "vision_route without={:?} with={:?} reason={}",
        without.model, with.model, with.reason
    );
}

#[test]
fn e2e_override_path_notifies() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("subagent-router.toml");
    std::fs::write(&path, STARTER_TOML).unwrap();

    let sensor = FixtureSensor {
        snaps: Default::default(),
        fail: Default::default(),
    };
    let spy = SpyNotifier::new();
    let d = resolve_for_spawn(
        &RouteInput {
            task_type: Some("design".into()),
            complexity: Some("high".into()),
            requires_vision: true,
            model_override: Some("claude-opus-5".into()),
        },
        Some("parent"),
        Some(&path),
        &sensor,
        &spy,
    );
    assert_eq!(d.source, RouteSource::ModelOverride);
    assert_eq!(d.model.as_deref(), Some("claude-opus-5"));
    assert_eq!(d.tool_ceiling, "general-purpose");
    let calls = spy.calls();
    assert!(!calls.is_empty());
    assert!(calls.iter().any(|c| c.1.contains("claude-opus-5")));
    println!("override_route notify_calls={calls:?}");
}

#[test]
fn e2e_scout_tool_ceiling_is_unrestricted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("subagent-router.toml");
    std::fs::write(&path, STARTER_TOML).unwrap();
    let sensor = FixtureSensor {
        snaps: Default::default(),
        fail: Default::default(),
    };
    let spy = SpyNotifier::new();
    let d = resolve_for_spawn(
        &RouteInput {
            task_type: Some("scout".into()),
            complexity: Some("low".into()),
            requires_vision: false,
            model_override: None,
        },
        Some("parent"),
        Some(&path),
        &sensor,
        &spy,
    );
    assert_eq!(d.tool_ceiling, "general-purpose");
    assert_eq!(d.source, RouteSource::Routed);
    println!("scout_route model={:?} ceiling={}", d.model, d.tool_ceiling);
}
