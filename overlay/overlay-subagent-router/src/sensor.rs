//! CodexBar usage sensor + process-wide TTL cache + command timeouts.

use std::collections::HashMap;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::auth_bridge::history_fallback_snapshot;
use crate::config::SensorConfig;
use crate::decision::UsageWindowSnap;

#[derive(Debug, Error)]
pub enum SensorError {
    #[error("spawn failed: {0}")]
    Spawn(String),
    #[error("timeout")]
    Timeout,
    #[error("provider error: {0}")]
    Provider(String),
    #[error("parse: {0}")]
    Parse(String),
}

#[derive(Debug, Clone)]
pub struct UsageSnapshot {
    pub provider: String,
    pub windows: Vec<UsageWindowSnap>,
    pub credits_remaining: Option<f64>,
}

pub trait Sensor: Send + Sync {
    fn fetch_provider(
        &self,
        cfg: &SensorConfig,
        provider: &str,
    ) -> Result<UsageSnapshot, SensorError>;
}

/// Live CodexBar CLI: `codexbar usage --format json --provider <id>`.
/// Honors `SensorConfig.timeout_secs` by killing the child when the deadline hits.
///
/// On live failure, falls back to the CodexBar menu-bar app's history snapshot
/// when `history_fallback_max_age_secs > 0` and a fresh sample exists.
pub struct CodexBarSensor;

impl Sensor for CodexBarSensor {
    fn fetch_provider(
        &self,
        cfg: &SensorConfig,
        provider: &str,
    ) -> Result<UsageSnapshot, SensorError> {
        match self.fetch_live(cfg, provider) {
            Ok(s) => Ok(s),
            Err(live_err) => {
                if cfg.history_fallback_max_age_secs == 0 {
                    return Err(live_err);
                }
                let max_age = Duration::from_secs(cfg.history_fallback_max_age_secs);
                match history_fallback_snapshot(provider, max_age) {
                    Ok(s) => {
                        tracing::warn!(
                            provider = %provider,
                            windows = s.windows.len(),
                            "codexbar live fetch failed; using menu-bar history fallback"
                        );
                        Ok(s)
                    }
                    Err(_history_error) => {
                        tracing::warn!(
                            provider = %provider,
                            "codexbar live + history fallback both failed"
                        );
                        Err(live_err)
                    }
                }
            }
        }
    }
}

impl CodexBarSensor {
    fn fetch_live(&self, cfg: &SensorConfig, provider: &str) -> Result<UsageSnapshot, SensorError> {
        // Prefer CodexBar's own OAuth source for Claude. The router never reads
        // Keychain items or copies credential files.
        if provider == "claude"
            && let Ok(s) = self.run_codexbar(cfg, provider, Some(&["--source", "oauth"]))
        {
            return Ok(s);
        }
        // Fall through to auto (may still hang; timeout will cut it).

        self.run_codexbar(cfg, provider, None)
    }

    fn run_codexbar(
        &self,
        cfg: &SensorConfig,
        provider: &str,
        extra: Option<&[&str]>,
    ) -> Result<UsageSnapshot, SensorError> {
        let mut args = cfg.usage_args.clone();
        args.push("--provider".into());
        args.push(provider.into());
        if let Some(extra) = extra {
            for a in extra {
                args.push((*a).into());
            }
        }
        // Per-provider extra args from config (e.g. force source).
        if let Some(extra_cfg) = cfg.provider_extra_args.get(provider) {
            for a in extra_cfg {
                args.push(a.clone());
            }
        }

        let mut cmd = Command::new(&cfg.command);
        cmd.args(&args);
        for (k, v) in &cfg.env {
            cmd.env(k, v);
        }

        let timeout = Duration::from_secs(cfg.timeout_secs.max(1));
        let output = output_with_timeout(cmd, timeout)?;

        if !output.status.success() {
            return Err(SensorError::Provider(format!(
                "codexbar exited with status {:?}",
                output.status.code()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_usage_json(provider, &stdout)
    }
}

/// Run a command and kill it if it exceeds `timeout`.
pub fn output_with_timeout(
    mut cmd: Command,
    timeout: Duration,
) -> Result<std::process::Output, SensorError> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    // Child is waited on and killed on timeout below.
    #[allow(clippy::disallowed_methods)]
    let mut child = cmd
        .spawn()
        .map_err(|e| SensorError::Spawn(format!("spawn: {e}")))?;

    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();

    let stdout_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(ref mut s) = stdout_pipe {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let stderr_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(ref mut s) = stderr_pipe {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = stdout_handle.join().unwrap_or_default();
                let stderr = stderr_handle.join().unwrap_or_default();
                return Ok(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    // Join readers so we don't leave them blocked forever.
                    let _ = stdout_handle.join();
                    let _ = stderr_handle.join();
                    return Err(SensorError::Timeout);
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(SensorError::Spawn(format!("wait: {e}")));
            }
        }
    }
}

/// In-memory TTL cache wrapping any sensor.
///
/// When built with [`CachedSensor::with_shared_cache`], the cache is process-wide
/// so successive spawns reuse CodexBar results within `ttl`.
pub struct CachedSensor<S: Sensor> {
    inner: S,
    cache: SharedCache,
    ttl: Duration,
}

enum SharedCache {
    Local(Mutex<MemoryCache>),
    Process,
}

pub struct MemoryCache {
    entries: HashMap<String, (Instant, UsageSnapshot)>,
}

impl MemoryCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

impl Default for MemoryCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Process-wide usage cache shared by every spawn in this process.
fn process_usage_cache() -> &'static Mutex<MemoryCache> {
    static CACHE: OnceLock<Mutex<MemoryCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(MemoryCache::new()))
}

/// Process-wide CodexBar sensor (cached). TTL is taken from the first caller.
pub fn process_global_codexbar_sensor(ttl_secs: u64) -> &'static CachedSensor<CodexBarSensor> {
    static SENSOR: OnceLock<CachedSensor<CodexBarSensor>> = OnceLock::new();
    SENSOR.get_or_init(|| CachedSensor::with_shared_cache(CodexBarSensor, ttl_secs))
}

impl<S: Sensor> CachedSensor<S> {
    /// Local cache owned by this instance (good for tests).
    pub fn new(inner: S, ttl_secs: u64) -> Self {
        Self {
            inner,
            cache: SharedCache::Local(Mutex::new(MemoryCache::new())),
            ttl: Duration::from_secs(ttl_secs.max(1)),
        }
    }

    /// Uses the process-wide cache so TTL survives across TaskTool spawns.
    pub fn with_shared_cache(inner: S, ttl_secs: u64) -> Self {
        Self {
            inner,
            cache: SharedCache::Process,
            ttl: Duration::from_secs(ttl_secs.max(1)),
        }
    }

    fn lock_cache(&self) -> std::sync::MutexGuard<'_, MemoryCache> {
        match &self.cache {
            SharedCache::Local(m) => m.lock().unwrap_or_else(|e| e.into_inner()),
            SharedCache::Process => process_usage_cache()
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        }
    }
}

impl<S: Sensor> Sensor for CachedSensor<S> {
    fn fetch_provider(
        &self,
        cfg: &SensorConfig,
        provider: &str,
    ) -> Result<UsageSnapshot, SensorError> {
        {
            let cache = self.lock_cache();
            if let Some((at, snap)) = cache.entries.get(provider)
                && at.elapsed() < self.ttl
            {
                return Ok(snap.clone());
            }
        }
        let snap = self.inner.fetch_provider(cfg, provider)?;
        {
            let mut cache = self.lock_cache();
            cache
                .entries
                .insert(provider.to_string(), (Instant::now(), snap.clone()));
        }
        Ok(snap)
    }
}

/// Fetch multiple providers **in parallel**; returns (ok snapshots, failed provider ids).
///
/// Parallelism matters: Claude's old auto path could burn the full per-provider
/// timeout alone and push the whole spawn past a minute. Concurrent probes keep
/// healthy providers (gemini/codex/grok) responsive while a bad one times out.
pub fn fetch_providers(
    sensor: &dyn Sensor,
    cfg: &SensorConfig,
    providers: &[String],
) -> (Vec<UsageSnapshot>, Vec<String>) {
    if providers.is_empty() {
        return (vec![], vec![]);
    }

    // Trait is Send + Sync; share via reference with scoped threads so one hung
    // provider does not serialize the others.
    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(providers.len());
        for p in providers {
            let provider = p.clone();
            handles.push(scope.spawn(move || {
                let result = sensor.fetch_provider(cfg, &provider);
                (provider, result)
            }));
        }

        let mut ok = Vec::new();
        let mut failed = Vec::new();
        let mut results: Vec<(String, Result<UsageSnapshot, SensorError>)> = handles
            .into_iter()
            .map(|h| {
                h.join().unwrap_or_else(|_| {
                    (
                        "unknown".into(),
                        Err(SensorError::Spawn("provider fetch thread panicked".into())),
                    )
                })
            })
            .collect();
        results.sort_by_key(|(p, _)| providers.iter().position(|x| x == p).unwrap_or(usize::MAX));
        for (p, result) in results {
            match result {
                Ok(s) => ok.push(s),
                Err(e) => {
                    tracing::warn!(provider = %p, error = %e, "codexbar provider fetch failed");
                    failed.push(p);
                }
            }
        }
        (ok, failed)
    })
}

/// Parse CodexBar usage JSON (array or single object).
pub fn parse_usage_json(provider_hint: &str, raw: &str) -> Result<UsageSnapshot, SensorError> {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).map_err(|e| SensorError::Parse(e.to_string()))?;

    let obj = if let Some(arr) = v.as_array() {
        arr.iter()
            .find(|o| o.get("provider").and_then(|p| p.as_str()) == Some(provider_hint))
            .cloned()
            .or_else(|| arr.first().cloned())
            .ok_or_else(|| SensorError::Parse("empty usage array".into()))?
    } else {
        v
    };

    if let Some(err) = obj.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("provider error");
        return Err(SensorError::Provider(msg.to_string()));
    }

    let provider = obj
        .get("provider")
        .and_then(|p| p.as_str())
        .unwrap_or(provider_hint)
        .to_string();

    let usage = obj.get("usage").cloned().unwrap_or(serde_json::Value::Null);
    let mut windows = Vec::new();
    for key in ["primary", "secondary", "tertiary"] {
        if let Some(w) = usage.get(key) {
            if w.is_null() {
                continue;
            }
            if let Some(snap) = parse_window(w) {
                windows.push(snap);
            }
        }
    }
    if let Some(extra) = usage.get("extraRateWindows").and_then(|e| e.as_array()) {
        for item in extra {
            let w = item.get("window").unwrap_or(item);
            if let Some(snap) = parse_window(w) {
                windows.push(snap);
            }
        }
    }

    let credits_remaining = obj
        .get("credits")
        .and_then(|c| c.get("remaining"))
        .and_then(|r| r.as_f64());

    Ok(UsageSnapshot {
        provider,
        windows,
        credits_remaining,
    })
}

fn parse_window(w: &serde_json::Value) -> Option<UsageWindowSnap> {
    let used = w.get("usedPercent")?.as_f64()?;
    let window_minutes = w
        .get("windowMinutes")
        .and_then(|m| m.as_u64().or_else(|| m.as_f64().map(|f| f as u64)));
    let resets_at = w
        .get("resetsAt")
        .and_then(|r| r.as_str())
        .map(|s| s.to_string());
    Some(UsageWindowSnap {
        used_percent: used,
        window_minutes,
        resets_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn parse_opencode_style() {
        let raw = r#"[{
          "provider":"opencodego",
          "usage":{
            "primary":{"windowMinutes":300,"usedPercent":13},
            "secondary":{"windowMinutes":10080,"usedPercent":5},
            "tertiary":{"windowMinutes":43200,"usedPercent":8}
          },
          "source":"web"
        }]"#;
        let s = parse_usage_json("opencodego", raw).unwrap();
        assert_eq!(s.provider, "opencodego");
        assert_eq!(s.windows.len(), 3);
    }

    #[test]
    fn parse_error_object() {
        let raw = r#"[{"provider":"claude","error":{"message":"timed out","code":1}}]"#;
        let err = parse_usage_json("claude", raw).unwrap_err();
        assert!(matches!(err, SensorError::Provider(_)));
    }

    #[test]
    fn parse_grok_style_without_window_minutes() {
        let raw = r#"[{
          "provider":"grok",
          "usage":{
            "primary":{"resetsAt":"2026-08-09T22:38:22Z","usedPercent":8},
            "secondary":null,
            "tertiary":null
          },
          "source":"grok-web"
        }]"#;
        let s = parse_usage_json("grok", raw).unwrap();
        assert_eq!(s.windows.len(), 1);
        assert!(s.windows[0].window_minutes.is_none());
        assert!((s.windows[0].used_percent - 8.0).abs() < 1e-9);
    }

    #[test]
    fn hung_command_is_cut_off_by_timeout() {
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        let start = Instant::now();
        let err = output_with_timeout(cmd, Duration::from_secs(1)).unwrap_err();
        assert!(matches!(err, SensorError::Timeout), "got {err:?}");
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "timeout should kill quickly, took {:?}",
            start.elapsed()
        );
    }

    struct CountingSensor {
        calls: Arc<AtomicUsize>,
    }

    impl Sensor for CountingSensor {
        fn fetch_provider(
            &self,
            _cfg: &SensorConfig,
            provider: &str,
        ) -> Result<UsageSnapshot, SensorError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(UsageSnapshot {
                provider: provider.into(),
                windows: vec![],
                credits_remaining: None,
            })
        }
    }

    #[test]
    fn second_fetch_within_ttl_does_not_rehit_inner() {
        let calls = Arc::new(AtomicUsize::new(0));
        let inner = CountingSensor {
            calls: calls.clone(),
        };
        let cached = CachedSensor::new(inner, 60);
        let cfg = SensorConfig::default();
        cached.fetch_provider(&cfg, "claude").unwrap();
        cached.fetch_provider(&cfg, "claude").unwrap();
        cached.fetch_provider(&cfg, "claude").unwrap();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "inner sensor must be called once within TTL"
        );
    }

    #[test]
    fn shared_process_cache_survives_separate_cached_sensor_instances() {
        let calls = Arc::new(AtomicUsize::new(0));
        // Clear process cache entries for this provider key by writing a fresh snapshot
        // under a unique provider name so tests don't collide with other tests.
        let provider = format!("test-ttl-{}", std::process::id());
        let cfg = SensorConfig::default();

        let s1 = CachedSensor::with_shared_cache(
            CountingSensor {
                calls: calls.clone(),
            },
            60,
        );
        s1.fetch_provider(&cfg, &provider).unwrap();

        let s2 = CachedSensor::with_shared_cache(
            CountingSensor {
                calls: calls.clone(),
            },
            60,
        );
        s2.fetch_provider(&cfg, &provider).unwrap();

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "process-wide cache must share hits across CachedSensor instances"
        );
    }
}
