//! Usage window classification and exhaustion.

use crate::config::WindowsConfig;
use crate::decision::WindowClass;

/// remaining% = 100 - usedPercent
pub fn remaining_percent(used_percent: f64) -> f64 {
    (100.0 - used_percent).clamp(0.0, 100.0)
}

pub fn window_exhausted(used_percent: f64, min_remaining_percent: f64) -> bool {
    remaining_percent(used_percent) <= min_remaining_percent
}

/// Classify by windowMinutes thresholds. Missing minutes → Unknown (still vetoable).
pub fn classify_window(window_minutes: Option<u64>, cfg: &WindowsConfig) -> WindowClass {
    let Some(m) = window_minutes else {
        return WindowClass::Unknown;
    };
    if m <= cfg.session_max_minutes {
        return WindowClass::Session;
    }
    if m >= cfg.monthly_min_minutes {
        return WindowClass::Monthly;
    }
    if m >= cfg.weekly_min_minutes && m <= cfg.weekly_max_minutes {
        return WindowClass::Weekly;
    }
    // Between session max and weekly min (e.g. ~12h) → treat as session-ish
    if m < cfg.weekly_min_minutes {
        return WindowClass::Session;
    }
    // Between weekly max and monthly min → weekly-ish
    WindowClass::Weekly
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaining_and_exhaustion() {
        assert!((remaining_percent(99.0) - 1.0).abs() < 1e-9);
        assert!(window_exhausted(99.5, 1.0));
        assert!(!window_exhausted(50.0, 1.0));
        assert!(window_exhausted(100.0, 1.0));
    }

    #[test]
    fn classify_thresholds() {
        let cfg = WindowsConfig::default();
        assert_eq!(classify_window(Some(300), &cfg), WindowClass::Session);
        assert_eq!(classify_window(Some(10080), &cfg), WindowClass::Weekly);
        assert_eq!(classify_window(Some(43200), &cfg), WindowClass::Monthly);
        assert_eq!(classify_window(None, &cfg), WindowClass::Unknown);
    }
}
