use std::time::Duration;

pub const KEY: &str = "reconciliation_minutes";

pub fn valid(value: &str) -> bool {
    value.parse::<u32>().is_ok_and(|minutes| minutes <= 10_080)
}

pub fn interval(watcher_available: bool, value: Option<&str>) -> Option<Duration> {
    if !watcher_available {
        return Some(Duration::from_secs(300));
    }
    let minutes = value
        .filter(|value| valid(value))
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1440);
    (minutes != 0).then(|| Duration::from_secs(minutes * 60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_watcher_uses_daily_default_instead_of_five_minutes() {
        assert_eq!(interval(true, None), Some(Duration::from_secs(86_400)));
        assert_eq!(interval(true, Some("60")), Some(Duration::from_secs(3_600)));
        assert_eq!(interval(true, Some("0")), None);
    }

    #[test]
    fn unavailable_watcher_always_keeps_five_minute_fallback() {
        for value in [None, Some("0"), Some("1440")] {
            assert_eq!(interval(false, value), Some(Duration::from_secs(300)));
        }
    }

    #[test]
    fn invalid_preferences_cannot_disable_recovery() {
        for value in ["", "-1", "1.5", "10081", "18446744073709551615"] {
            assert!(!valid(value));
            assert_eq!(interval(true, Some(value)), interval(true, None));
        }
        for value in ["0", "1", "1440", "10080"] {
            assert!(valid(value));
        }
    }
}
