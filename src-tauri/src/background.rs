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
    watch_retry_at: Instant,
    recovery_pending: bool,
    reconciled_at: Instant,
    last: Option<Instant>,
    events: u64,
    failures: u32,
    retry_at: Instant,
}
fn initial_work(
    engine: &foldersync_engine::Engine,
    task: &Task,
    startup: bool,
) -> anyhow::Result<Option<Instant>> {
    if task.monitor || (startup && task.login) {
        engine.queue_paths(&task.id, &[], true)?;
    }
    // The persisted next occurrence may already have advanced before shutdown.
    // Resume its durable work independently of whether another tick is due.
    Ok((!engine.queued(&task.id)?.is_empty()).then(Instant::now))
}
pub fn start(app: tauri::AppHandle, r: Runtime) {
    std::thread::spawn(move || {
        let mut watched: HashMap<String, Watched> = HashMap::new();
        let mut startup = true;
        loop {
            std::thread::sleep(Duration::from_millis(250));
            let (tasks, safety_interval) = match r.engine.lock().ok().and_then(|e| {
                Some((
                    e.tasks().ok()?,
                    e.setting(crate::reconciliation::KEY).ok().flatten(),
                ))
            }) {
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
                    let last = match r.engine.lock() {
                        Ok(e) => {
                            if !e
                                .tasks()
                                .ok()
                                .is_some_and(|tasks| tasks.iter().any(|current| current == &t))
                            {
                                continue;
                            }
                            match initial_work(&e, &t, startup) {
                                Ok(last) => last,
                                Err(error) => {
                                    let _ = app
                                        .emit("background-error", format!("{}: {error:#}", t.name));
                                    continue;
                                }
                            }
                        }
                        Err(_) => continue,
                    };
                    watched.insert(
                        t.id.clone(),
                        Watched {
                            task: t.clone(),
                            watch,
                            watch_retry_at: Instant::now() + Duration::from_secs(30),
                            recovery_pending: false,
                            reconciled_at: Instant::now(),
                            last,
                            events: 0,
                            failures: 0,
                            retry_at: Instant::now(),
                        },
                    );
                }
                let w = watched.get_mut(&t.id).unwrap();
                if t.monitor && w.watch.is_none() && Instant::now() >= w.watch_retry_at {
                    w.watch_retry_at = Instant::now() + Duration::from_secs(30);
                    if let Ok(watch) = WatchService::new(&t) {
                        w.watch = Some(watch);
                        // Notifications cannot reconstruct changes while disconnected.
                        w.recovery_pending = true;
                    }
                }
                if due.contains(&t.id) {
                    w.last = Some(Instant::now() - Duration::from_secs(t.debounce_secs));
                }
                let mut batch = w.watch.as_ref().map(|w| w.poll()).unwrap_or_default();
                if batch.watch_lost {
                    w.watch = None;
                    w.watch_retry_at = Instant::now() + Duration::from_secs(30);
                    let _ = app.emit(
                        "background-error",
                        format!(
                            "{}: watcher interrupted; reconnecting with reconciliation fallback",
                            t.name
                        ),
                    );
                }
                let periodic = t.monitor
                    && crate::reconciliation::interval(
                        w.watch.is_some(),
                        safety_interval.as_deref(),
                    )
                    .is_some_and(|interval| w.reconciled_at.elapsed() >= interval);
                if periodic || w.recovery_pending {
                    batch.uncertain = true;
                }
                if batch.uncertain || !batch.paths.is_empty() {
                    let queued = if let Ok(e) = r.engine.lock() {
                        if !e
                            .tasks()
                            .ok()
                            .is_some_and(|tasks| tasks.iter().any(|current| current == &t))
                        {
                            continue;
                        }
                        e.queue_watch_batch(&t.id, &batch)
                    } else {
                        Err(anyhow::anyhow!("Engine lock failed"))
                    };
                    if let Err(error) = queued {
                        // poll drains memory, but failed persistence must never
                        // silently lose those paths or rename hints.
                        w.recovery_pending = true;
                        let _ = app.emit(
                            "background-error",
                            format!("{}: queue persistence failed: {error:#}", t.name),
                        );
                        continue;
                    }
                    w.recovery_pending = false;
                    if batch.uncertain {
                        w.reconciled_at = Instant::now();
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
                    .map_err(|_| anyhow::anyhow!("Engine lock failed"))
                    .and_then(|mut e| {
                        if !e.tasks()?.iter().any(|current| current == &t) {
                            return Ok(None);
                        }
                        e.process_queue(&t.id)
                    });
                match result {
                    Ok(Some(plan)) => {
                        if plan.metrics.reconciliations > 0 {
                            w.reconciled_at = Instant::now();
                        }
                        w.events = 0;
                        w.failures = 0;
                        let pending = r
                            .engine
                            .lock()
                            .ok()
                            .and_then(|e| e.pending_conflicts(&t.id).ok())
                            .unwrap_or(0);
                        let attention = pending > 0
                            || plan.operations.iter().any(|o| o.action == Action::Conflict)
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
                            let signature = serde_json::to_string(&(
                                plan.status.clone(),
                                plan.operations
                                    .iter()
                                    .filter(|o| o.action != Action::Unchanged)
                                    .map(|o| {
                                        (
                                            &o.path,
                                            o.action,
                                            o.a.as_ref().map(|e| &e.digest),
                                            o.b.as_ref().map(|e| &e.digest),
                                        )
                                    })
                                    .collect::<Vec<_>>(),
                            ))
                            .unwrap_or_default();
                            let changed = r.engine.lock().ok().is_some_and(|e| {
                                let key = format!("notification/{}", t.id);
                                let old = e.setting(&key).ok().flatten();
                                let changed = old.as_deref() != Some(signature.as_str());
                                if changed {
                                    let _ = e.set_setting(&key, &signature);
                                }
                                changed
                            });
                            if enabled && changed {
                                let de = r
                                    .engine
                                    .lock()
                                    .ok()
                                    .and_then(|e| e.setting("language").ok().flatten())
                                    .as_deref()
                                    == Some("de");
                                let (title, body) = crate::presentation::notice(
                                    de,
                                    if attention {
                                        crate::presentation::NoticeKind::Review
                                    } else {
                                        crate::presentation::NoticeKind::Success
                                    },
                                    &t.name,
                                    pending,
                                );
                                let _ = app
                                    .notification()
                                    .builder()
                                    .extra("task_id", &t.id)
                                    .title(title)
                                    .body(body)
                                    .sound("Default")
                                    .show();
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(error) if error.is::<foldersync_engine::Cancelled>() => {
                        // Stop cancels this attempt. Preserve durable work for a later
                        // event/schedule/reconciliation instead of retrying immediately.
                        w.failures = 0;
                        let _ = app.emit(
                            "run-finished",
                            serde_json::json!({"task":t.id,"status":"Cancelled","time":now()}),
                        );
                    }
                    Err(error) => {
                        w.failures += 1;
                        let _ = app.emit("background-error", format!("{}: {}", t.name, error));
                        if w.failures <= t.retries {
                            let delay = Duration::from_secs((1u64 << w.failures.min(6)) * 2);
                            w.retry_at = Instant::now() + delay;
                            w.last = Some(Instant::now());
                        } else if r
                            .engine
                            .lock()
                            .ok()
                            .and_then(|e| e.setting("notifications").ok().flatten())
                            .as_deref()
                            != Some("off")
                        {
                            let de = r
                                .engine
                                .lock()
                                .ok()
                                .and_then(|e| e.setting("language").ok().flatten())
                                .as_deref()
                                == Some("de");
                            let (title, body) = crate::presentation::notice(
                                de,
                                crate::presentation::NoticeKind::Failure,
                                &t.name,
                                0,
                            );
                            let _ = app
                                .notification()
                                .builder()
                                .extra("task_id", &t.id)
                                .title(title)
                                .body(body)
                                .sound("Default")
                                .show();
                        }
                    }
                }
            }
            startup = false;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use foldersync_engine::{Engine, Schedule};
    use std::{fs, path::PathBuf};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "FolderSync-runtime-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn scheduled_queue_resumes_after_restart_before_next_occurrence() {
        let fixture = Fixture::new();
        let task = Task {
            a: fixture.0.join("A"),
            b: fixture.0.join("B"),
            schedule: Schedule {
                kind: "periodic".into(),
                interval_minutes: 60,
                ..Schedule::default()
            },
            ..Task::default()
        };
        fs::create_dir(&task.a).unwrap();
        fs::create_dir(&task.b).unwrap();
        fs::write(task.a.join("f"), "queued before restart").unwrap();
        let state = fixture.0.join("state");
        let mut engine = Engine::open(&state).unwrap();
        engine.save_task(&task).unwrap();
        engine.schedule_tick(1_800_000_000).unwrap();
        assert_eq!(
            engine.schedule_tick(1_800_003_600).unwrap(),
            vec![task.id.clone()]
        );
        drop(engine);
        let mut engine = Engine::open(&state).unwrap();
        assert!(engine.schedule_tick(1_800_003_601).unwrap().is_empty());
        assert!(
            initial_work(&engine, &task, true).unwrap().is_some(),
            "A durable queued occurrence must wake the restarted runtime"
        );
        engine.process_queue(&task.id).unwrap();
        assert_eq!(
            fs::read(task.b.join("f")).unwrap(),
            b"queued before restart"
        );
        assert!(
            initial_work(&engine, &task, true).unwrap().is_none(),
            "Completed scheduled work must not rerun merely because the runtime starts"
        );
    }
}
