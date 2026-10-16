use foldersync_engine::{ProgressPhase, RunProgress};

pub fn tooltip(
    de: bool,
    paused: bool,
    progress: Option<&RunProgress>,
    name: Option<&str>,
) -> String {
    let text = if let Some(progress) = progress {
        let phase = match (de, progress.phase) {
            (true, ProgressPhase::Discovering) => "Dateien vergleichen",
            (true, ProgressPhase::Validating) => "Änderungen prüfen",
            (true, ProgressPhase::Applying) => "Dateien synchronisieren",
            (true, ProgressPhase::Transferring) => "Dateien übertragen",
            (true, ProgressPhase::Protecting) => "Dateiversion sichern",
            (true, ProgressPhase::Verifying) => "Übertragung prüfen",
            (false, ProgressPhase::Discovering) => "Comparing files",
            (false, ProgressPhase::Validating) => "Checking changes",
            (false, ProgressPhase::Applying) => "Synchronizing files",
            (false, ProgressPhase::Transferring) => "Transferring files",
            (false, ProgressPhase::Protecting) => "Protecting file version",
            (false, ProgressPhase::Verifying) => "Verifying transfer",
        };
        let status = if de { "Aufgabe läuft" } else { "Job running" };
        let name = name
            .filter(|name| !name.trim().is_empty())
            .map(|name| format!(" · {name}"))
            .unwrap_or_default();
        format!("FolderSync — {status}: {phase}{name}")
    } else {
        match (de, paused) {
            (true, true) => "FolderSync — Automatik pausiert · Kein aktiver Auftrag",
            (true, false) => "FolderSync — Kein aktiver Auftrag",
            (false, true) => "FolderSync — Automation paused · No active job",
            (false, false) => "FolderSync — No active job",
        }
        .into()
    };
    let text: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if text.encode_utf16().count() <= 127 {
        return text;
    }
    let mut units = 0;
    let mut shortened: String = text
        .chars()
        .take_while(|c| {
            units += c.len_utf16();
            units <= 126
        })
        .collect();
    shortened.push('…');
    shortened
}

#[cfg(test)]
mod tests {
    use super::*;
    use foldersync_engine::ProgressPhase;

    fn progress(phase: ProgressPhase) -> RunProgress {
        RunProgress {
            task_id: "task".into(),
            run_id: None,
            phase,
            completed_entries: 0,
            total_entries: 1,
            path: None,
            file_bytes: 0,
            file_total: 0,
            active_transfers: 0,
            max_transfers: 4,
        }
    }

    #[test]
    fn running_job_then_idle_and_pause_are_distinct() {
        let running = tooltip(
            true,
            false,
            Some(&progress(ProgressPhase::Transferring)),
            Some("Lukas Test"),
        );
        assert!(running.contains("Aufgabe läuft"));
        assert!(running.contains("Lukas Test"));
        assert!(running.contains("Dateien übertragen"));
        assert!(tooltip(true, false, None, None).contains("Kein aktiver Auftrag"));
        assert!(tooltip(true, true, None, None).contains("Automatik pausiert"));
        assert!(
            tooltip(
                true,
                true,
                Some(&progress(ProgressPhase::Transferring)),
                None
            )
            .contains("Aufgabe läuft"),
            "Pausing automation does not stop an active job"
        );
    }

    #[test]
    fn every_phase_is_localized_without_claiming_idle() {
        for phase in [
            ProgressPhase::Discovering,
            ProgressPhase::Validating,
            ProgressPhase::Applying,
            ProgressPhase::Transferring,
            ProgressPhase::Protecting,
            ProgressPhase::Verifying,
        ] {
            let value = progress(phase);
            let de = tooltip(true, false, Some(&value), None);
            let en = tooltip(false, false, Some(&value), None);
            assert!(de.contains("Aufgabe läuft"));
            assert!(en.contains("Job running"));
            assert_ne!(de, en);
        }
    }

    #[test]
    fn tooltip_fits_windows_utf16_limit_and_sanitizes_task_names() {
        let name = format!("Fixture\n{}", "🦀".repeat(100));
        let text = tooltip(
            true,
            false,
            Some(&progress(ProgressPhase::Discovering)),
            Some(&name),
        );
        assert!(text.contains("Fixture"));
        assert!(!text.chars().any(char::is_control));
        assert!(text.encode_utf16().count() <= 127);
        assert!(text.ends_with('…'));
    }
}
