#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    Success,
    Review,
    Failure,
}
pub fn test_notice(de: bool) -> (String, String) {
    if de {
        (
            "FolderSync — Testbenachrichtigung".into(),
            "Test mit Standardsound. Windows steuert Banner und Ton über die Benachrichtigungseinstellungen und Nicht stören.".into(),
        )
    } else {
        (
            "FolderSync — notification test".into(),
            "Test with the default sound. Windows controls banners and sound through notification settings and Do not disturb.".into(),
        )
    }
}
pub fn menu_labels(de: bool) -> [(&'static str, &'static str); 4] {
    if de {
        [
            ("open", "FolderSync öffnen"),
            ("pause", "Automatik pausieren / fortsetzen"),
            ("stop", "Aktiven Lauf stoppen"),
            ("quit", "Beenden"),
        ]
    } else {
        [
            ("open", "Open FolderSync"),
            ("pause", "Pause / Resume automation"),
            ("stop", "Stop active work"),
            ("quit", "Quit"),
        ]
    }
}
pub fn notice(de: bool, kind: NoticeKind, name: &str, pending: u64) -> (String, String) {
    let (title, body) = match (de, kind) {
        (true, NoticeKind::Success) => (
            "FolderSync — synchronisiert",
            format!("{name}: Übertragungen abgeschlossen. {pending} offene Konflikte."),
        ),
        (true, NoticeKind::Review) => (
            "FolderSync — Prüfung erforderlich",
            format!(
                "{name}: Prüfung oder Nacharbeiten erforderlich. {pending} offene Konflikte. Details öffnen."
            ),
        ),
        (true, NoticeKind::Failure) => (
            "FolderSync — Synchronisation fehlgeschlagen",
            format!(
                "{name}: Wiederholungsversuche ausgeschöpft. Details öffnen und erneut prüfen."
            ),
        ),
        (false, NoticeKind::Success) => (
            "FolderSync — synchronized",
            format!("{name}: transfers completed. {pending} pending conflicts."),
        ),
        (false, NoticeKind::Review) => (
            "FolderSync — review required",
            format!(
                "{name}: review or follow-up required. {pending} pending conflicts. Open Details to inspect."
            ),
        ),
        (false, NoticeKind::Failure) => (
            "FolderSync — synchronization failed",
            format!("{name}: retries exhausted. Open Details and preview again."),
        ),
    };
    (title.into(), body)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_changes_labels_without_changing_tray_action_identity() {
        assert_eq!(
            menu_labels(true).map(|x| x.0),
            menu_labels(false).map(|x| x.0)
        );
        assert_eq!(menu_labels(true)[0].1, "FolderSync öffnen");
    }
    #[test]
    fn conflict_and_failure_notices_are_distinct_in_both_languages() {
        for de in [false, true] {
            let review = notice(de, NoticeKind::Review, "Fixture", 2);
            let failed = notice(de, NoticeKind::Failure, "Fixture", 0);
            assert_ne!(review.0, failed.0);
            assert!(review.1.contains('2'));
            assert!(failed.1.contains("Fixture"));
            assert_ne!(review.0, notice(de, NoticeKind::Success, "Fixture", 0).0);
        }
    }
    #[test]
    fn test_notice_explains_windows_delivery_without_claiming_a_banner() {
        let (english_title, english_body) = test_notice(false);
        let (german_title, german_body) = test_notice(true);
        assert!(english_title.contains("test"));
        assert!(german_title.contains("Test"));
        assert!(english_body.contains("Windows"));
        assert!(german_body.contains("Windows"));
        assert_ne!(english_body, german_body);
    }
}
