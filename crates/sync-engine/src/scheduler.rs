use crate::Schedule;
use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, Datelike, Duration, LocalResult, TimeZone};
pub fn next_due<T: TimeZone>(s: &Schedule, after: DateTime<T>) -> Result<Option<i64>> {
    if s.kind.is_empty() {
        return Ok(None);
    }
    if s.kind == "periodic" {
        ensure!(
            (1..=525600).contains(&s.interval_minutes),
            "Invalid periodic interval"
        );
        return Ok(Some(after.timestamp() + i64::from(s.interval_minutes) * 60));
    }
    ensure!(s.hour < 24 && s.minute < 60, "Invalid schedule time");
    ensure!(
        matches!(s.kind.as_str(), "daily" | "weekly" | "monthly"),
        "Unknown schedule kind"
    );
    if s.kind == "weekly" {
        ensure!(
            !s.weekdays.is_empty() && s.weekdays.iter().all(|d| *d < 7),
            "Select valid weekdays"
        );
    }
    if s.kind == "monthly" {
        ensure!((1..=31).contains(&s.month_day), "Invalid month day");
    }
    let zone = after.timezone();
    let mut date = after.date_naive();
    for _ in 0..=370 {
        let eligible = match s.kind.as_str() {
            "daily" => true,
            "weekly" => s.weekdays.contains(&date.weekday().num_days_from_sunday()),
            "monthly" => date.day() == s.month_day,
            _ => false,
        };
        if eligible {
            let local = date
                .and_hms_opt(s.hour, s.minute, 0)
                .context("Invalid calendar time")?;
            for shift in 0..=180 {
                let candidate = local + Duration::minutes(shift);
                let instant = match zone.from_local_datetime(&candidate) {
                    LocalResult::Single(d) => Some(d),
                    LocalResult::Ambiguous(a, b) => {
                        Some(if a.timestamp() < b.timestamp() { a } else { b })
                    }
                    LocalResult::None => None,
                };
                if let Some(instant) = instant {
                    if instant.timestamp() > after.timestamp() {
                        return Ok(Some(instant.timestamp()));
                    }
                    break;
                }
            }
        }
        date = date.succ_opt().context("Calendar overflow")?;
    }
    bail!("No calendar occurrence found in the next year")
}
