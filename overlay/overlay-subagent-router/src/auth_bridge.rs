//! Read-only CodexBar history fallback.
//!
//! When live `codexbar usage` fails, the router may use the menu-bar app's
//! already persisted history snapshots under
//! `~/Library/Application Support/com.steipete.codexbar/history/`.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::decision::UsageWindowSnap;
use crate::sensor::{SensorError, UsageSnapshot};

/// Load the newest CodexBar history sample for `provider` if fresh enough.
pub fn history_fallback_snapshot(
    provider: &str,
    max_age: Duration,
) -> Result<UsageSnapshot, SensorError> {
    let path = history_path(provider)?;
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| SensorError::Provider(format!("history {}: {e}", path.display())))?;
    parse_history_json(provider, &raw, max_age, SystemTime::now())
}

fn history_path(provider: &str) -> Result<PathBuf, SensorError> {
    let home = dirs::home_dir().ok_or_else(|| SensorError::Spawn("no home dir".into()))?;
    Ok(home
        .join("Library/Application Support/com.steipete.codexbar/history")
        .join(format!("{provider}.json")))
}

/// Parse CodexBar history JSON (`unscoped` window series with `entries`).
pub fn parse_history_json(
    provider: &str,
    raw: &str,
    max_age: Duration,
    now: SystemTime,
) -> Result<UsageSnapshot, SensorError> {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).map_err(|e| SensorError::Parse(e.to_string()))?;

    let series = v
        .get("unscoped")
        .and_then(|u| u.as_array())
        .ok_or_else(|| SensorError::Parse("history missing unscoped[]".into()))?;

    let mut windows = Vec::new();
    let mut newest_captured: Option<SystemTime> = None;

    for s in series {
        let window_minutes = s
            .get("windowMinutes")
            .and_then(|m| m.as_u64().or_else(|| m.as_f64().map(|f| f as u64)));
        let entries = match s.get("entries").and_then(|e| e.as_array()) {
            Some(e) if !e.is_empty() => e,
            _ => continue,
        };
        let last = entries.last().unwrap();
        let used = last
            .get("usedPercent")
            .and_then(|u| u.as_f64())
            .ok_or_else(|| SensorError::Parse("history entry missing usedPercent".into()))?;
        let resets_at = last
            .get("resetsAt")
            .and_then(|r| r.as_str())
            .map(|s| s.to_string());
        if let Some(cap) = last.get("capturedAt").and_then(|c| c.as_str()) {
            if let Ok(ts) = parse_rfc3339(cap) {
                newest_captured = Some(match newest_captured {
                    Some(prev) => prev.max(ts),
                    None => ts,
                });
            }
        }
        windows.push(UsageWindowSnap {
            used_percent: used,
            window_minutes,
            resets_at,
        });
    }

    if windows.is_empty() {
        return Err(SensorError::Parse("history has no usable windows".into()));
    }

    if let Some(captured) = newest_captured {
        let age = now
            .duration_since(captured)
            .unwrap_or(Duration::from_secs(0));
        if age > max_age {
            return Err(SensorError::Provider(format!(
                "history stale (age {}s > {}s)",
                age.as_secs(),
                max_age.as_secs()
            )));
        }
    }

    Ok(UsageSnapshot {
        provider: provider.to_string(),
        windows,
        credits_remaining: None,
    })
}

fn parse_rfc3339(s: &str) -> Result<SystemTime, ()> {
    // Accept `2026-08-04T11:58:20Z` without pulling chrono.
    // Format: YYYY-MM-DDTHH:MM:SSZ (seconds precision is enough for freshness).
    let s = s.trim().trim_end_matches('Z');
    let (date, time) = s.split_once('T').ok_or(())?;
    let mut dparts = date.split('-');
    let y: i32 = dparts.next().ok_or(())?.parse().map_err(|_| ())?;
    let mo: u32 = dparts.next().ok_or(())?.parse().map_err(|_| ())?;
    let d: u32 = dparts.next().ok_or(())?.parse().map_err(|_| ())?;
    let mut tparts = time.split(':');
    let h: u32 = tparts.next().ok_or(())?.parse().map_err(|_| ())?;
    let mi: u32 = tparts.next().ok_or(())?.parse().map_err(|_| ())?;
    let se_s = tparts.next().ok_or(())?;
    let se: u32 = se_s
        .split(['.', '+'])
        .next()
        .ok_or(())?
        .parse()
        .map_err(|_| ())?;

    // Days from civil date (Howard Hinnant algorithm) → unix seconds.
    let y = if mo <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era as i64 * 146097 + doe as i64 - 719468;
    let secs = days * 86400 + (h as i64) * 3600 + (mi as i64) * 60 + se as i64;
    if secs < 0 {
        return Err(());
    }
    Ok(UNIX_EPOCH + Duration::from_secs(secs as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_opencode_history_fresh() {
        let raw = r#"{
          "unscoped": [
            {
              "name": "session",
              "windowMinutes": 300,
              "entries": [
                {"capturedAt": "2026-08-04T11:58:20Z", "resetsAt": "2026-08-04T16:58:19Z", "usedPercent": 0}
              ]
            },
            {
              "name": "weekly",
              "windowMinutes": 10080,
              "entries": [
                {"capturedAt": "2026-08-04T11:58:20Z", "resetsAt": "2026-08-09T23:59:59Z", "usedPercent": 5}
              ]
            }
          ],
          "version": 1
        }"#;
        // now = capturedAt + 60s
        let now = UNIX_EPOCH
            + Duration::from_secs(
                // 2026-08-04T11:59:20Z
                parse_rfc3339("2026-08-04T11:59:20Z")
                    .unwrap()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            );
        let snap = parse_history_json("opencodego", raw, Duration::from_secs(900), now).unwrap();
        assert_eq!(snap.provider, "opencodego");
        assert_eq!(snap.windows.len(), 2);
        assert!((snap.windows[1].used_percent - 5.0).abs() < 1e-9);
        assert_eq!(snap.windows[0].window_minutes, Some(300));
    }

    #[test]
    fn parse_history_rejects_stale() {
        let raw = r#"{
          "unscoped": [
            {
              "windowMinutes": 300,
              "entries": [
                {"capturedAt": "2026-08-01T00:00:00Z", "usedPercent": 1}
              ]
            }
          ]
        }"#;
        let now = UNIX_EPOCH
            + Duration::from_secs(
                parse_rfc3339("2026-08-04T12:00:00Z")
                    .unwrap()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            );
        let err = parse_history_json("opencodego", raw, Duration::from_secs(900), now).unwrap_err();
        assert!(matches!(err, SensorError::Provider(_)), "{err:?}");
    }

    #[test]
    fn rfc3339_roundtrip_smoke() {
        let t = parse_rfc3339("2026-08-04T11:58:20Z").unwrap();
        assert!(t > UNIX_EPOCH);
    }
}
