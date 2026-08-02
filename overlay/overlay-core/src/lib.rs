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
}
