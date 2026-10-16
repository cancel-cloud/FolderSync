use super::Runtime;
use foldersync_engine::{Action, Task, now, watch::WatchService};
use std::{
    collections::HashMap,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use tauri::Emitter;
use tauri_plugin_notification::NotificationExt;
struct Watched {
    task: Task,
    watch: Option<WatchService>,
    reconciled_at: Instant,
    last: Option<Instant>,
    events: u64,
    failures: u32,
    retry_at: Instant,
}
pub fn start(app: tauri::AppHandle, r: Runtime) {
    std::thread::spawn(move || {
        let mut watched: HashMap<String, Watched> = HashMap::new();
        let mut startup = true;
        loop {
            std::thread::sleep(Duration::from_millis(250));
            let tasks = match r.engine.lock().ok().and_then(|e| e.tasks().ok()) {
                Some(t) => t,
                None => continue,
            };
            let due = if !r.paused.load(Ordering::Relaxed) {
                r.engine
                    .lock()
                    .ok()
                    .and_then(|e| e.schedule_tick(now() as i64).ok())
                    .unwrap_or_default()
            } else {
                vec![]
            };
            watched.retain(|id, _| {
                tasks
                    .iter()
                    .any(|t| t.id == *id && (t.monitor || t.login || !t.schedule.kind.is_empty()))
            });
            for t in tasks
                .into_iter()
                .filter(|t| t.monitor || t.login || !t.schedule.kind.is_empty())
            {
                if watched.get(&t.id).is_none_or(|w| w.task != t) {
                    let watch = if t.monitor {
                        match WatchService::new(&t) {
                            Ok(w) => Some(w),
                            Err(e) => {
                                let _=app.emit("background-error",format!("{}: watcher unavailable; 5-minute reconciliation fallback: {e}",t.name));
                                None
                            }
                        }
                    } else {
                        None
                    };
                    if let Ok(e) = r.engine.lock()
                        && (t.monitor || (startup && t.login))
                    {
                        let _ = e.queue_paths(&t.id, &[], true);
                    }
                    watched.insert(
                        t.id.clone(),
                        Watched {
                            task: t.clone(),
                            watch,
                            reconciled_at: Instant::now(),
                            last: if t.monitor || (startup && t.login) {
                                Some(Instant::now())
                            } else {
                                None
                            },
                            events: 0,
                            failures: 0,
                            retry_at: Instant::now(),
                        },
                    );
                }
                let w = watched.get_mut(&t.id).unwrap();
                if due.contains(&t.id) {
                    w.last = Some(Instant::now() - Duration::from_secs(t.debounce_secs));
                }
                let mut batch = w.watch.as_ref().map(|w| w.poll()).unwrap_or_default();
                if t.monitor && w.reconciled_at.elapsed() >= Duration::from_secs(300) {
                    batch.uncertain = true;
                    w.reconciled_at = Instant::now();
                }
                if batch.uncertain || !batch.paths.is_empty() {
                    if let Ok(e) = r.engine.lock() {
                        let _ = e.queue_paths(&t.id, &batch.paths, batch.uncertain);
                    }
                    w.last = Some(Instant::now());
                    w.events += batch.events;
                }
                if r.paused.load(Ordering::Relaxed) || Instant::now() < w.retry_at {
                    continue;
                }
                let due = w
                    .last
                    .is_some_and(|x| x.elapsed() >= Duration::from_secs(t.debounce_secs.max(1)));
                if !due {
                    continue;
                }
                w.last = None;
                let result = r
                    .engine
                    .lock()
                    .map_err(|_| "Engine lock failed".to_string())
                    .and_then(|mut e| e.process_queue(&t.id).map_err(|e| format!("{e:#}")));
                match result {
                    Ok(Some(mut plan)) => {
                        plan.metrics.watch_events = w.events;
                        w.events = 0;
                        w.failures = 0;
                        let attention =
                            plan.operations.iter().any(|o| o.action == Action::Conflict)
                                || plan.status == "Review required";
                        let meaningful = plan
                            .operations
                            .iter()
                            .any(|o| o.action != Action::Unchanged);
                        if meaningful {
                            let _=app.emit("run-finished",serde_json::json!({"task":t.id,"run":plan.id,"status":plan.status,"time":now()}));
                            let enabled = r
                                .engine
                                .lock()
                                .ok()
                                .and_then(|e| e.setting("notifications").ok().flatten())
                                .as_deref()
                                != Some("off");
                            if enabled {
                                let title = if attention {
                                    "FolderSync — review required"
                                } else {
                                    "FolderSync — synchronized"
                                };
                                let _ = app
                                    .notification()
                                    .builder()
                                    .title(title)
                                    .body(format!("{}: {}", t.name, plan.status))
                                    .show();
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        w.failures += 1;
                        let _ = app.emit("background-error", format!("{}: {}", t.name, error));
                        if w.failures <= t.retries {
                            let delay = Duration::from_secs((1u64 << w.failures.min(6)) * 2);
                            w.retry_at = Instant::now() + delay;
                            w.last = Some(Instant::now());
                        } else {
                            let _ = app
                                .notification()
                                .builder()
                                .title("FolderSync — synchronization failed")
                                .body(format!(
                                    "{}: retries exhausted. Open Details and preview again.",
                                    t.name
                                ))
                                .show();
                        }
                    }
                }
            }
            startup = false;
        }
    });
}
