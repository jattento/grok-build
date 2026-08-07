//! Router TOML config load + seed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

/// Embedded starter written to `~/.grok/subagent-router.toml` when missing.
pub const STARTER_TOML: &str = include_str!("../../subagent-router/subagent-router.starter.toml");

pub const DEFAULT_CONFIG_REL: &str = "subagent-router.toml";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml: {0}")]
    Toml(#[from] toml::de::Error),
}

#[derive(Debug, Clone, Deserialize)]
pub struct RouterConfig {
    #[serde(default)]
    pub sensor: SensorConfig,
    #[serde(default)]
    pub windows: WindowsConfig,
    #[serde(default)]
    pub tool_ceiling: HashMap<String, String>,
    #[serde(default)]
    pub fallback: FallbackConfig,
    #[serde(default, rename = "override")]
    pub override_cfg: OverrideConfig,
    #[serde(default)]
    pub vision: VisionConfig,
    #[serde(default)]
    pub models: HashMap<String, ModelMeta>,
    #[serde(default)]
    pub routes: HashMap<String, HashMap<String, RouteCell>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SensorConfig {
    #[serde(default = "default_codexbar")]
    pub command: String,
    #[serde(default = "default_usage_args")]
    pub usage_args: Vec<String>,
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_secs: u64,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_true")]
    pub notify_on_provider_error: bool,
    /// `auto` | `osascript` | `terminal-notifier` | `none` | custom binary.
    #[serde(default = "default_notify_command")]
    pub notify_command: String,
    /// Extra env vars injected into every `codexbar` spawn.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Extra argv per provider (appended after `--provider <id>`).
    #[serde(default)]
    pub provider_extra_args: HashMap<String, Vec<String>>,
    /// When live CodexBar fails, accept menu-bar history samples this fresh
    /// (seconds). `0` disables the history fallback.
    #[serde(default = "default_history_max_age")]
    pub history_fallback_max_age_secs: u64,
}

impl Default for SensorConfig {
    fn default() -> Self {
        Self {
            command: default_codexbar(),
            usage_args: default_usage_args(),
            cache_ttl_secs: default_cache_ttl(),
            timeout_secs: default_timeout(),
            notify_on_provider_error: true,
            notify_command: default_notify_command(),
            env: HashMap::new(),
            provider_extra_args: HashMap::new(),
            history_fallback_max_age_secs: default_history_max_age(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct WindowsConfig {
    #[serde(default = "default_min_remaining")]
    pub min_remaining_percent: f64,
    #[serde(default = "default_session_max")]
    pub session_max_minutes: u64,
    #[serde(default = "default_weekly_min")]
    pub weekly_min_minutes: u64,
    #[serde(default = "default_weekly_max")]
    pub weekly_max_minutes: u64,
    #[serde(default = "default_monthly_min")]
    pub monthly_min_minutes: u64,
    #[serde(default = "default_true")]
    pub include_extra_rate_windows: bool,
    #[serde(default = "default_true")]
    pub credits_are_informational: bool,
    #[serde(default = "default_rank_primary")]
    pub rank_primary: String,
    #[serde(default = "default_rank_secondary")]
    pub rank_secondary: String,
}

impl Default for WindowsConfig {
    fn default() -> Self {
        Self {
            min_remaining_percent: default_min_remaining(),
            session_max_minutes: default_session_max(),
            weekly_min_minutes: default_weekly_min(),
            weekly_max_minutes: default_weekly_max(),
            monthly_min_minutes: default_monthly_min(),
            include_extra_rate_windows: true,
            credits_are_informational: true,
            rank_primary: default_rank_primary(),
            rank_secondary: default_rank_secondary(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FallbackConfig {
    #[serde(default = "default_true")]
    pub use_parent_model: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OverrideConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub notify_on_use: bool,
    #[serde(default)]
    pub require_prior_failure: bool,
}

impl Default for OverrideConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            notify_on_use: true,
            require_prior_failure: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct VisionConfig {
    #[serde(default)]
    pub default_requires_vision: bool,
    #[serde(default = "default_fallback_parent")]
    pub fallback_when_none: String,
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            default_requires_vision: false,
            fallback_when_none: default_fallback_parent(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelMeta {
    pub provider: String,
    #[serde(default)]
    pub supports_reasoning_effort: bool,
    #[serde(default = "default_true")]
    pub supports_vision: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RouteCell {
    pub models: Vec<String>,
    #[serde(default)]
    pub effort: Option<String>,
}

impl RouterConfig {
    pub fn route_cell(&self, task_type: &str, complexity: &str) -> Option<&RouteCell> {
        self.routes.get(task_type)?.get(complexity)
    }

    pub fn tool_ceiling(&self, task_type: &str) -> &str {
        self.tool_ceiling
            .get(task_type)
            .map(|s| s.as_str())
            .unwrap_or_else(|| crate::decision::tool_ceiling_for_task_type(task_type))
    }
}

pub fn resolve_config_path() -> PathBuf {
    if let Ok(p) = std::env::var("GROK_SUBAGENT_ROUTER_CONFIG") {
        return PathBuf::from(p);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".grok")
        .join(DEFAULT_CONFIG_REL)
}

pub fn seed_config_if_missing(path: &Path) -> Result<bool, ConfigError> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, STARTER_TOML)?;
    Ok(true)
}

pub fn load_config(path: &Path) -> Result<RouterConfig, ConfigError> {
    let text = std::fs::read_to_string(path)?;
    load_config_from_str(&text)
}

pub fn load_config_from_str(text: &str) -> Result<RouterConfig, ConfigError> {
    Ok(toml::from_str(text)?)
}

fn default_codexbar() -> String {
    "codexbar".into()
}
fn default_usage_args() -> Vec<String> {
    vec!["usage".into(), "--format".into(), "json".into()]
}
fn default_cache_ttl() -> u64 {
    90
}
fn default_timeout() -> u64 {
    // Live OAuth (claude) and web providers finish in a few seconds. 25s is
    // enough to fail a hung CLI PTY without blocking the whole spawn long.
    25
}
fn default_notify_command() -> String {
    "auto".into()
}
fn default_history_max_age() -> u64 {
    // CodexBar app only appends history when a live probe succeeds. When OpenCode
    // cookies break, samples freeze for hours. 12h keeps the provider eligible
    // with a slightly stale weekly signal instead of dropping it entirely.
    43200
}
fn default_true() -> bool {
    true
}
fn default_min_remaining() -> f64 {
    1.0
}
fn default_session_max() -> u64 {
    360
}
fn default_weekly_min() -> u64 {
    1440
}
fn default_weekly_max() -> u64 {
    11520
}
fn default_monthly_min() -> u64 {
    40320
}
fn default_rank_primary() -> String {
    "weekly".into()
}
fn default_rank_secondary() -> String {
    "session".into()
}
fn default_fallback_parent() -> String {
    "parent".into()
}
