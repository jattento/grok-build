//! macOS (and spy) notifications.
//!
//! Bare `osascript display notification` often "succeeds" (exit 0) while the
//! banner is suppressed for the Script Editor identity. We:
//! 1. Prefer `terminal-notifier` when present (proper NC identity).
//! 2. Fall back to `/usr/bin/osascript` with an audible `sound name`.
//! 3. Always kick `afplay` of a system sound so failures are still audible.
//! 4. Log every attempt (success / failure) via `tracing` — callers no longer
//!    swallow errors silently.

use std::io;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;

pub trait Notifier: Send + Sync {
    fn notify(&self, title: &str, body: &str) -> io::Result<()>;
}

/// macOS notifier: terminal-notifier → osascript (+ Glass sound) → afplay.
pub struct OsascriptNotifier {
    /// Base command name or path. `"osascript"` / `"auto"` / `"terminal-notifier"` / `"none"`.
    pub command: String,
}

impl Default for OsascriptNotifier {
    fn default() -> Self {
        Self {
            command: "auto".into(),
        }
    }
}

impl Notifier for OsascriptNotifier {
    fn notify(&self, title: &str, body: &str) -> io::Result<()> {
        if self.command == "none" {
            tracing::info!(%title, %body, "notify suppressed (notify_command=none)");
            return Ok(());
        }

        // Always play a short system sound so the event is audible even when
        // Notification Center banners are disabled for Script Editor.
        spawn_afplay_glass();

        let mode = self.command.as_str();
        let result = match mode {
            "terminal-notifier" => notify_terminal_notifier(title, body),
            "osascript" => notify_osascript(title, body),
            "auto" | "" => {
                if which_exists("terminal-notifier") {
                    notify_terminal_notifier(title, body).or_else(|e| {
                        tracing::warn!(error = %e, "terminal-notifier failed; falling back to osascript");
                        notify_osascript(title, body)
                    })
                } else {
                    notify_osascript(title, body)
                }
            }
            other => {
                // Treat unknown values as an explicit binary path/name for osascript-compat.
                let mut cmd = Command::new(other);
                let script = osascript_script(title, body);
                let status = cmd.args(["-e", &script]).status()?;
                if status.success() {
                    Ok(())
                } else {
                    Err(io::Error::other(format!(
                        "notify command {other:?} exited {:?}",
                        status.code()
                    )))
                }
            }
        };

        match &result {
            Ok(()) => tracing::info!(%title, %body, "macOS notification posted"),
            Err(e) => tracing::error!(%title, %body, error = %e, "macOS notification failed"),
        }
        result
    }
}

fn osascript_script(title: &str, body: &str) -> String {
    format!(
        "display notification \"{}\" with title \"{}\" sound name \"Glass\"",
        escape_applescript(body),
        escape_applescript(title)
    )
}

fn notify_osascript(title: &str, body: &str) -> io::Result<()> {
    let script = osascript_script(title, body);
    // Prefer absolute path — PATH can be sparse under the leader process.
    let bin = if std::path::Path::new("/usr/bin/osascript").exists() {
        "/usr/bin/osascript"
    } else {
        "osascript"
    };
    let status = Command::new(bin).args(["-e", &script]).status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "osascript exited {:?}",
            status.code()
        )));
    }
    Ok(())
}

fn notify_terminal_notifier(title: &str, body: &str) -> io::Result<()> {
    let status = Command::new("terminal-notifier")
        .args([
            "-title",
            title,
            "-message",
            body,
            "-sound",
            "Glass",
            // Group so successive router alerts replace each other.
            "-group",
            "grok-subagent-router",
        ])
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "terminal-notifier exited {:?}",
            status.code()
        )));
    }
    Ok(())
}

fn spawn_afplay_glass() {
    thread::spawn(|| {
        let sound = "/System/Library/Sounds/Glass.aiff";
        if !std::path::Path::new(sound).exists() {
            return;
        }
        let _ = Command::new("afplay")
            .arg(sound)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    });
}

fn which_exists(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Records notifications for tests.
#[derive(Default)]
pub struct SpyNotifier {
    pub calls: Mutex<Vec<(String, String)>>,
}

impl SpyNotifier {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn calls(&self) -> Vec<(String, String)> {
        self.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }
}

impl Notifier for SpyNotifier {
    fn notify(&self, title: &str, body: &str) -> io::Result<()> {
        if let Ok(mut c) = self.calls.lock() {
            c.push((title.to_string(), body.to_string()));
        }
        Ok(())
    }
}

pub fn notify_override(notifier: &dyn Notifier, model: &str) -> io::Result<()> {
    notifier.notify(
        "Grok subagent override",
        &format!("Error-path model override: {model}"),
    )
}

pub fn notify_provider_error(
    notifier: &dyn Notifier,
    provider: &str,
    detail: &str,
) -> io::Result<()> {
    notifier.notify(
        "Grok CodexBar error",
        &format!("Provider {provider}: {detail}"),
    )
}
