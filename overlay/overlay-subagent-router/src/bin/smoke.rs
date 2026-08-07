//! Smoke binary for the subagent router (no pager deps).
use overlay_subagent_router::{
    RouteInput, RouteSource, STARTER_TOML, Sensor, SensorConfig, SensorError, SpyNotifier,
    UsageSnapshot, resolve_config_path, resolve_for_spawn, seed_config_if_missing,
};

struct EmptySensor;
impl Sensor for EmptySensor {
    fn fetch_provider(
        &self,
        _cfg: &SensorConfig,
        provider: &str,
    ) -> Result<UsageSnapshot, SensorError> {
        Ok(UsageSnapshot {
            provider: provider.into(),
            windows: vec![],
            credits_remaining: None,
        })
    }
}

fn main() {
    let path = resolve_config_path();
    let _ = seed_config_if_missing(&path);
    let spy = SpyNotifier::new();
    let sensor = EmptySensor;

    // (a) normal route
    let d1 = resolve_for_spawn(
        &RouteInput {
            task_type: Some("implement".into()),
            complexity: Some("medium".into()),
            requires_vision: false,
            model_override: None,
        },
        Some("parent-model"),
        Some(&path),
        &sensor,
        &spy,
    );
    println!(
        "normal: source={:?} model={:?} effort={:?} ceiling={}",
        d1.source, d1.model, d1.effort, d1.tool_ceiling
    );
    assert_eq!(d1.tool_ceiling, "general-purpose");
    assert!(matches!(
        d1.source,
        RouteSource::Routed | RouteSource::ParentFallback
    ));

    // (b) vision
    let d2 = resolve_for_spawn(
        &RouteInput {
            task_type: Some("implement".into()),
            complexity: Some("high".into()),
            requires_vision: true,
            model_override: None,
        },
        Some("parent-model"),
        Some(&path),
        &sensor,
        &spy,
    );
    println!(
        "vision: source={:?} model={:?} ceiling={}",
        d2.source, d2.model, d2.tool_ceiling
    );

    // (c) override + notify
    let spy2 = SpyNotifier::new();
    let d3 = resolve_for_spawn(
        &RouteInput {
            task_type: Some("design".into()),
            complexity: Some("high".into()),
            requires_vision: false,
            model_override: Some("claude-opus-5".into()),
        },
        Some("parent-model"),
        Some(&path),
        &sensor,
        &spy2,
    );
    assert_eq!(d3.source, RouteSource::ModelOverride);
    assert_eq!(d3.model.as_deref(), Some("claude-opus-5"));
    assert!(!spy2.calls().is_empty());
    println!("override: model={:?} notify={:?}", d3.model, spy2.calls());

    // Prove starter parses
    let _ = overlay_subagent_router::load_config_from_str(STARTER_TOML).expect("starter");
    println!("smoke_ok");
}
