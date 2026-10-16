use foldersync_engine::*;
use std::{
    fs,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

#[test]
fn cancellation_reports_actual_run_and_preserves_destination_during_staging() {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-cancel-progress-")
        .tempdir()
        .unwrap();
    let mut t = Task {
        a: d.path().join("A"),
        b: d.path().join("B"),
        ..Task::default()
    };
    fs::create_dir(&t.a).unwrap();
    fs::create_dir(&t.b).unwrap();
    fs::write(t.a.join("f.bin"), b"original").unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    e.save_task(&t).unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    t.bandwidth_kib = 512;
    e.set_setting("log_level", "basic").unwrap();
    e.save_task(&t).unwrap();
    fs::write(t.a.join("f.bin"), vec![42; 8 * 1024 * 1024]).unwrap();
    let p = e.preview(&t.id).unwrap();
    let progress = e.progress.clone();
    let cancel = e.cancel.clone();
    let expected_run = p.id.clone();
    let observer = std::thread::spawn(move || {
        let start = Instant::now();
        loop {
            if let Some(snapshot) = progress.snapshot()
                && snapshot.phase == ProgressPhase::Transferring
                && snapshot.file_bytes > 0
            {
                assert_eq!(snapshot.run_id.as_deref(), Some(expected_run.as_str()));
                assert!(snapshot.file_bytes < snapshot.file_total);
                cancel.store(true, Ordering::Relaxed);
                return snapshot;
            }
            if start.elapsed() > Duration::from_secs(5) {
                cancel.store(true, Ordering::Relaxed);
                panic!("No live transfer progress observed");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let result = e.apply_reviewed(&p.id, false).unwrap();
    let snapshot = observer.join().unwrap();
    assert_eq!(snapshot.path.as_deref(), Some("f.bin"));
    assert_eq!(result.plan.status, "Cancelled");
    assert!(result.error.unwrap().contains("Cancelled"));
    assert!(e.progress.snapshot().is_none());
    assert_eq!(fs::read(t.b.join("f.bin")).unwrap(), b"original");
    assert_eq!(
        fs::metadata(t.a.join("f.bin")).unwrap().len(),
        8 * 1024 * 1024
    );
    assert!(!fs::read_dir(&t.b).unwrap().any(|r| {
        r.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".foldersync-stage-")
    }));
    assert_eq!(e.history_headers(&t.id).unwrap()[0].status, "Cancelled");
    assert_eq!(
        e.history(&t.id).unwrap()[0]
            .operations
            .iter()
            .find(|o| o.path == "f.bin")
            .unwrap()
            .status,
        "Cancelled"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(e.recent_log().unwrap().trim()).unwrap()["status"],
        "Cancelled"
    );
    t.bandwidth_kib = 0;
    e.save_task(&t).unwrap();
    let retry = e.preview(&t.id).unwrap();
    let retry = e.apply_reviewed(&retry.id, false).unwrap();
    assert!(retry.error.is_none());
    assert_eq!(
        fs::read(t.a.join("f.bin")).unwrap(),
        fs::read(t.b.join("f.bin")).unwrap()
    );
}

#[test]
fn rejected_preview_reports_error_without_changing_any_file() {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-rejected-apply-")
        .tempdir()
        .unwrap();
    let t = Task {
        a: d.path().join("A"),
        b: d.path().join("B"),
        ..Task::default()
    };
    fs::create_dir(&t.a).unwrap();
    fs::create_dir(&t.b).unwrap();
    fs::write(t.a.join("f"), "before preview").unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    e.save_task(&t).unwrap();
    let p = e.preview(&t.id).unwrap();
    fs::write(t.a.join("f"), "after preview").unwrap();
    let result = e.apply_reviewed(&p.id, false).unwrap();
    assert!(result.plan.status.starts_with("Rejected:"));
    assert_ne!(result.plan.id, p.id);
    assert_eq!(e.history_headers(&t.id).unwrap()[0].id, result.plan.id);
    assert!(result.error.unwrap().contains("Stale input"));
    assert!(!t.b.join("f").exists());
    assert_eq!(fs::read_to_string(t.a.join("f")).unwrap(), "after preview");
    assert!(e.progress.snapshot().is_none());
}

#[test]
fn preview_and_apply_validation_can_cancel_inside_streaming_hashes() {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-hash-cancel-")
        .tempdir()
        .unwrap();
    let t = Task {
        a: d.path().join("A"),
        b: d.path().join("B"),
        ..Task::default()
    };
    fs::create_dir(&t.a).unwrap();
    fs::create_dir(&t.b).unwrap();
    fs::write(t.a.join("large.bin"), vec![27; 32 * 1024 * 1024]).unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    e.save_task(&t).unwrap();
    for phase in [ProgressPhase::Discovering, ProgressPhase::Validating] {
        let preview = if phase == ProgressPhase::Validating {
            Some(e.preview(&t.id).unwrap())
        } else {
            None
        };
        let progress = e.progress.clone();
        let cancel = e.cancel.clone();
        let observer = std::thread::spawn(move || {
            let start = Instant::now();
            loop {
                if let Some(value) = progress.snapshot()
                    && value.phase == phase
                    && value.file_bytes > 0
                    && value.file_bytes < value.file_total
                {
                    cancel.store(true, Ordering::Relaxed);
                    return value;
                }
                if start.elapsed() > Duration::from_secs(5) {
                    cancel.store(true, Ordering::Relaxed);
                    panic!("Missing streaming hash progress");
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        if let Some(p) = preview {
            let result = e.apply_reviewed(&p.id, false).unwrap();
            assert_eq!(result.plan.status, "Cancelled");
            assert_ne!(result.plan.id, p.id);
            assert!(result.error.unwrap().contains("Cancelled"));
        } else {
            assert!(e.preview(&t.id).is_err());
            let history = e.history_headers(&t.id).unwrap();
            assert_eq!(history[0].status, "Cancelled");
            assert!(history[0].metrics.hashed_bytes > 0);
        }
        let observed = observer.join().unwrap();
        assert_eq!(observed.path.as_deref(), Some("large.bin"));
        assert!(e.progress.snapshot().is_none());
        assert!(!t.b.join("large.bin").exists());
        assert_eq!(
            fs::metadata(t.a.join("large.bin")).unwrap().len(),
            32 * 1024 * 1024
        );
    }
}

#[test]
fn background_validation_cancellation_is_persisted_and_keeps_durable_work() {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-background-cancel-")
        .tempdir()
        .unwrap();
    let t = Task {
        a: d.path().join("A"),
        b: d.path().join("B"),
        ..Task::default()
    };
    fs::create_dir(&t.a).unwrap();
    fs::create_dir(&t.b).unwrap();
    fs::write(t.a.join("large.bin"), vec![83; 32 * 1024 * 1024]).unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    e.save_task(&t).unwrap();
    e.queue_paths(&t.id, &[], true).unwrap();
    let progress = e.progress.clone();
    let cancel = e.cancel.clone();
    let observer = std::thread::spawn(move || {
        let start = Instant::now();
        loop {
            if let Some(value) = progress.snapshot()
                && value.phase == ProgressPhase::Validating
                && value.file_bytes > 0
                && value.file_bytes < value.file_total
            {
                cancel.store(true, Ordering::Relaxed);
                return;
            }
            if start.elapsed() > Duration::from_secs(5) {
                cancel.store(true, Ordering::Relaxed);
                panic!("No background validation progress");
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    assert!(e.process_queue(&t.id).is_err());
    observer.join().unwrap();
    assert_eq!(e.history_headers(&t.id).unwrap()[0].status, "Cancelled");
    assert!(!e.queued(&t.id).unwrap().is_empty());
    assert!(!t.b.join("large.bin").exists());
}
