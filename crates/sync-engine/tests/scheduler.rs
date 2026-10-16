use chrono::{TimeZone, Utc};
use chrono_tz::Europe::Berlin;
use foldersync_engine::{Schedule, scheduler::next_due};
#[test]
fn periodic_calendar_and_month_skips() {
    let now = Utc.with_ymd_and_hms(2026, 1, 31, 12, 0, 0).unwrap();
    let mut s = Schedule {
        kind: "periodic".into(),
        interval_minutes: 15,
        ..Schedule::default()
    };
    assert_eq!(next_due(&s, now).unwrap(), Some(now.timestamp() + 900));
    s.kind = "monthly".into();
    s.month_day = 31;
    s.hour = 9;
    let result = next_due(&s, now).unwrap().unwrap();
    assert_eq!(
        result,
        Utc.with_ymd_and_hms(2026, 3, 31, 9, 0, 0)
            .unwrap()
            .timestamp()
    );
}
#[test]
fn dst_gap_advances_and_repeated_hour_runs_once() {
    let s = Schedule {
        kind: "daily".into(),
        hour: 2,
        minute: 30,
        ..Schedule::default()
    };
    let after = Berlin.with_ymd_and_hms(2026, 3, 28, 23, 0, 0).unwrap();
    assert_eq!(
        next_due(&s, after).unwrap(),
        Some(
            Berlin
                .with_ymd_and_hms(2026, 3, 29, 3, 0, 0)
                .unwrap()
                .timestamp()
        )
    );
    let first = Berlin
        .with_ymd_and_hms(2026, 10, 25, 2, 30, 0)
        .earliest()
        .unwrap();
    let next = next_due(&s, first).unwrap().unwrap();
    assert_eq!(
        next,
        Berlin
            .with_ymd_and_hms(2026, 10, 26, 2, 30, 0)
            .unwrap()
            .timestamp()
    );
}
