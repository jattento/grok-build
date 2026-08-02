//! Entry point for fork-local behaviour.
//!
//! Upstream (`xai-org/grok-build`) is synced as wholesale snapshots, so every
//! line we add to `crates/` is a future merge conflict. All custom logic lives
//! here instead; the upstream tree only calls into it.
//!
//! See `AGENTS.md` at the repo root and `overlay/TOUCHPOINTS.md`.

/// Version of the overlay itself, independent from the upstream grok version.
pub const OVERLAY_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Env var that suppresses the startup banner when set to a falsy value.
const BANNER_ENV: &str = "GROK_OVERLAY_BANNER";

/// Called once from the binary's composition root, before anything else runs.
///
/// This is the single upstream touchpoint the overlay owns. Add new wiring
/// inside this function rather than adding new call sites in `crates/`.
pub fn install() {
    if banner_enabled(std::env::var(BANNER_ENV).ok().as_deref()) {
        eprintln!("[overlay {OVERLAY_VERSION}] active");
    }
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

#[cfg(test)]
mod tests {
    use super::banner_enabled;

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
}
