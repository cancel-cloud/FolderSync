use crate::presentation::NoticeKind;
use foldersync_engine::Engine;

pub const KEY: &str = "notification_problems_only";

pub fn enabled(engine: &Engine, kind: NoticeKind) -> bool {
    let notifications = engine.setting("notifications").ok().flatten();
    let problems_only = engine.setting(KEY).ok().flatten();
    notifications.as_deref() != Some("off")
        && (kind != NoticeKind::Success || problems_only.as_deref() == Some("off"))
}

pub fn plan_kind(status: &str, pending: u64, conflicts: bool) -> NoticeKind {
    if status.starts_with("Failed") {
        NoticeKind::Failure
    } else if status != "Applied" || pending > 0 || conflicts {
        NoticeKind::Review
    } else {
        NoticeKind::Success
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_persisted_preferences_suppress_only_success() {
        let directory = tempfile::tempdir().unwrap();
        let engine = Engine::open(directory.path()).unwrap();
        assert!(!enabled(&engine, NoticeKind::Success));
        assert!(enabled(&engine, NoticeKind::Review));
        assert!(enabled(&engine, NoticeKind::Failure));
        engine.set_setting(KEY, "off").unwrap();
        drop(engine);
        let engine = Engine::open(directory.path()).unwrap();
        assert!(enabled(&engine, NoticeKind::Success));
        engine.set_setting(KEY, "on").unwrap();
        assert!(!enabled(&engine, NoticeKind::Success));
        assert!(enabled(&engine, NoticeKind::Review));
        assert!(enabled(&engine, NoticeKind::Failure));
        engine.set_setting("notifications", "off").unwrap();
        for kind in [NoticeKind::Success, NoticeKind::Review, NoticeKind::Failure] {
            assert!(!enabled(&engine, kind));
        }
        engine.set_setting(KEY, "off").unwrap();
        assert!(!enabled(&engine, NoticeKind::Success));
    }

    #[test]
    fn cleanup_warnings_conflicts_and_review_never_become_success() {
        assert_eq!(plan_kind("Applied", 0, false), NoticeKind::Success);
        assert_eq!(plan_kind("Applied", 1, false), NoticeKind::Review);
        assert_eq!(plan_kind("Applied", 0, true), NoticeKind::Review);
        for status in [
            "Attention",
            "Review required",
            "Applied; retention cleanup pending: locked",
        ] {
            assert_eq!(plan_kind(status, 0, false), NoticeKind::Review);
        }
        assert_eq!(
            plan_kind("Failed: destination locked", 0, false),
            NoticeKind::Failure
        );
    }
}
