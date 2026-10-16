use foldersync_engine::{ProgressState, RunProgress};
use tauri::Emitter;

fn changed(
    last: &mut Option<RunProgress>,
    current: Option<RunProgress>,
) -> Option<Option<RunProgress>> {
    if *last == current {
        return None;
    }
    *last = current.clone();
    Some(current)
}
pub fn start(app: tauri::AppHandle, progress: ProgressState) {
    // Dedicated sampler: the engine mutex is held during long filesystem work.
    // Intermediate snapshots are overwritten, never accumulated in a queue.
    std::thread::spawn(move || {
        let mut last = None;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(200));
            if let Some(value) = changed(&mut last, progress.snapshot()) {
                let _ = app.emit("run-progress", value);
            }
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sampler_emits_only_changes_and_emits_idle_after_work() {
        let mut last = None;
        assert!(changed(&mut last, None).is_none());
        let value = RunProgress {
            task_id: "task".into(),
            run_id: Some("run".into()),
            phase: foldersync_engine::ProgressPhase::Transferring,
            completed_entries: 0,
            total_entries: 1,
            path: Some("f".into()),
            file_bytes: 256,
            file_total: 1024,
            active_transfers: 1,
            max_transfers: 1,
        };
        assert_eq!(
            changed(&mut last, Some(value.clone())),
            Some(Some(value.clone()))
        );
        assert!(changed(&mut last, Some(value)).is_none());
        assert_eq!(changed(&mut last, None), Some(None));
    }
}
