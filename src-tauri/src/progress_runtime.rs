use foldersync_engine::{ProgressState, RunProgress};
use std::{
    collections::HashMap,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
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
pub fn start(app: tauri::AppHandle, runtime: super::Runtime, mut de: bool) {
    // Dedicated sampler: the engine mutex is held during long filesystem work.
    // Intermediate snapshots are overwritten, never accumulated in a queue.
    let progress: ProgressState = runtime.progress.clone();
    let mut names: HashMap<String, String> = runtime
        .engine
        .try_lock()
        .ok()
        .and_then(|engine| engine.tasks().ok())
        .unwrap_or_default()
        .into_iter()
        .map(|task| (task.id, task.name))
        .collect();
    std::thread::spawn(move || {
        let mut last = None;
        let mut last_tooltip = String::new();
        let mut refresh_at = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(200));
            // Metadata is best-effort; never wait on the engine mutex held by
            // a scan or transfer. The progress snapshot remains independent.
            if Instant::now() >= refresh_at {
                refresh_at = Instant::now() + Duration::from_secs(1);
                if let Ok(engine) = runtime.engine.try_lock() {
                    if let Ok(tasks) = engine.tasks() {
                        names = tasks.into_iter().map(|task| (task.id, task.name)).collect();
                    }
                    if let Ok(language) = engine.setting("language") {
                        de = language.as_deref() == Some("de");
                    }
                }
            }
            let current = progress.snapshot();
            let name = current
                .as_ref()
                .and_then(|p| names.get(&p.task_id))
                .map(String::as_str);
            let tooltip = crate::tray_status::tooltip(
                de,
                runtime.paused.load(Ordering::Relaxed),
                current.as_ref(),
                name,
            );
            if tooltip != last_tooltip
                && let Some(tray) = app.tray_by_id("main-tray")
                && tray.set_tooltip(Some(&tooltip)).is_ok()
            {
                last_tooltip = tooltip;
            }
            if let Some(value) = changed(&mut last, current) {
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
