use foldersync_engine::*;
use std::{
    fs,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

fn fixture(mode: Mode, workers: u32, bandwidth: u64) -> (tempfile::TempDir, Engine, Task) {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-parallel-")
        .tempdir()
        .unwrap();
    let mut t = Task {
        a: d.path().join("A"),
        b: d.path().join("B"),
        mode,
        bandwidth_kib: bandwidth,
        ..Task::default()
    };
    let mut json = serde_json::to_value(&t).unwrap();
    json["transfer_concurrency"] = workers.into();
    t = serde_json::from_value(json).unwrap();
    fs::create_dir(&t.a).unwrap();
    fs::create_dir(&t.b).unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    e.save_task(&t).unwrap();
    (d, e, t)
}
fn active_stages(root: &std::path::Path) -> usize {
    fs::read_dir(root)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".foldersync-stage-")
                && fs::File::open(entry.path())
                    .and_then(|file| file.metadata())
                    .is_ok_and(|m| m.len() > 0 && m.len() < 1024 * 1024)
        })
        .count()
}

#[test]
fn two_real_partial_stages_cancel_cleanly_without_worker_startup_recovery() {
    partial_stages_cancel(2);
    partial_stages_cancel(4);
}
fn partial_stages_cancel(workers: u32) {
    let (d, mut e, t) = fixture(Mode::TwoWay, workers, 512);
    for name in ["f0", "f1", "f2", "f3"] {
        fs::write(t.a.join(name), vec![42; 1024 * 1024]).unwrap();
    }
    let p = e.preview(&t.id).unwrap();
    let cancel = e.cancel.clone();
    let root = t.b.clone();
    let state = d.path().join("state/foldersync.sqlite3");
    let run = p.id.clone();
    let observer = std::thread::spawn(move || {
        let start = Instant::now();
        let mut count = 0;
        let mut status = String::new();
        while start.elapsed() < Duration::from_secs(3) {
            count = count.max(active_stages(&root));
            if count >= workers as usize {
                let db = rusqlite::Connection::open(&state).unwrap();
                status = db
                    .query_row("SELECT status FROM runs WHERE id=?", [run], |row| {
                        row.get(0)
                    })
                    .unwrap();
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        cancel.store(true, Ordering::Relaxed);
        (count, status)
    });
    let result = e.apply_reviewed(&p.id, false).unwrap();
    let (count, status) = observer.join().unwrap();
    assert_eq!(
        count, workers as usize,
        "configured destination stages must overlap"
    );
    assert_eq!(
        status, "Applying",
        "worker connections must not run startup recovery"
    );
    assert_eq!(result.plan.status, "Cancelled");
    for name in ["f0", "f1", "f2", "f3"] {
        assert_eq!(fs::read(t.a.join(name)).unwrap(), vec![42; 1024 * 1024]);
        assert!(!t.b.join(name).exists());
    }
    assert_eq!(fs::read_dir(&t.b).unwrap().count(), 0);
    let db = rusqlite::Connection::open(d.path().join("state/foldersync.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM staging", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        0
    );
}

#[test]
fn bounded_parallel_transfers_preserve_all_modes_and_directory_order() {
    for mode in [Mode::TwoWay, Mode::Copy, Mode::Mirror, Mode::Move] {
        let (_d, mut e, t) = fixture(mode, 2, 1024);
        fs::create_dir_all(t.a.join("nested/deep")).unwrap();
        for i in 0..4 {
            fs::write(
                t.a.join(format!("nested/deep/f{i}")),
                vec![i as u8; 256 * 1024],
            )
            .unwrap();
        }
        let p = e.preview(&t.id).unwrap();
        let p = e.apply(&p.id, true).unwrap();
        assert_eq!(p.status, "Applied");
        assert_eq!(serde_json::to_value(&p.metrics).unwrap()["peak_workers"], 2);
        for i in 0..4 {
            assert_eq!(
                fs::read(t.b.join(format!("nested/deep/f{i}"))).unwrap(),
                vec![i as u8; 256 * 1024]
            );
        }
        if mode == Mode::Move {
            assert_eq!(fs::read_dir(&t.a).unwrap().count(), 0);
        }
        let repeated = e.preview(&t.id).unwrap();
        assert!(
            repeated
                .operations
                .iter()
                .all(|o| o.action == Action::Unchanged)
        );
    }
}

#[test]
fn parallel_workers_share_the_task_bandwidth_budget() {
    let (_d, mut e, t) = fixture(Mode::Copy, 2, 1024);
    for i in 0..2 {
        fs::write(t.a.join(format!("f{i}")), vec![i as u8; 512 * 1024]).unwrap();
    }
    let p = e.preview(&t.id).unwrap();
    let start = Instant::now();
    e.apply(&p.id, false).unwrap();
    assert!(
        start.elapsed() >= Duration::from_millis(900),
        "1 MiB at a total 1 MiB/s must not get a separate budget per worker"
    );
}

#[test]
fn invalid_worker_counts_are_rejected_without_changing_saved_task() {
    let (_d, mut e, t) = fixture(Mode::Copy, 1, 0);
    for count in [0, 5] {
        let mut j = serde_json::to_value(&t).unwrap();
        j["transfer_concurrency"] = count.into();
        let bad = serde_json::from_value(j).unwrap();
        assert!(e.save_task(&bad).is_err());
    }
    assert_eq!(e.tasks().unwrap()[0], t);
}

#[test]
fn a_parallel_destination_race_preserves_external_bytes_and_prevents_following_deletes() {
    let (d, mut e, t) = fixture(Mode::Mirror, 2, 1024);
    for i in 0..2 {
        fs::write(t.a.join(format!("f{i}")), vec![i as u8; 1024 * 1024]).unwrap();
    }
    fs::write(t.b.join("extra"), b"must survive failed run").unwrap();
    let p = e.preview(&t.id).unwrap();
    let root = t.b.clone();
    let cancel = e.cancel.clone();
    let observer = std::thread::spawn(move || {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(3) {
            if active_stages(&root) == 2 {
                fs::write(root.join("f1"), b"external writer owns this").unwrap();
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        cancel.store(true, Ordering::Relaxed);
        false
    });
    let result = e.apply_reviewed(&p.id, true).unwrap();
    assert!(observer.join().unwrap());
    assert!(result.plan.status.starts_with("Failed:"));
    assert_eq!(
        fs::read(t.b.join("f1")).unwrap(),
        b"external writer owns this"
    );
    assert_eq!(
        fs::read(t.b.join("extra")).unwrap(),
        b"must survive failed run"
    );
    for i in 0..2 {
        assert_eq!(
            fs::read(t.a.join(format!("f{i}"))).unwrap(),
            vec![i as u8; 1024 * 1024]
        );
    }
    if t.b.join("f0").exists() {
        assert_eq!(fs::read(t.b.join("f0")).unwrap(), vec![0; 1024 * 1024]);
    }
    assert_eq!(
        e.history(&t.id).unwrap()[0]
            .operations
            .iter()
            .find(|o| o.path == "extra")
            .unwrap()
            .status,
        "Not run"
    );
    let db = rusqlite::Connection::open(d.path().join("state/foldersync.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM staging", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        0
    );
}

#[test]
fn concurrent_replacements_keep_each_old_version_and_commit_each_baseline() {
    let (_d, mut e, t) = fixture(Mode::TwoWay, 2, 0);
    for i in 0..4 {
        fs::write(t.a.join(format!("f{i}")), format!("old{i}")).unwrap();
    }
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    for i in 0..4 {
        fs::write(t.a.join(format!("f{i}")), format!("updated{i}")).unwrap();
    }
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    let versions = e.versions(&t.id).unwrap();
    for i in 0..4 {
        assert_eq!(
            fs::read(t.b.join(format!("f{i}"))).unwrap(),
            format!("updated{i}").as_bytes()
        );
        assert!(versions.iter().any(|v| v.path == format!("f{i}")
            && fs::read(&v.stored).unwrap() == format!("old{i}").as_bytes()));
    }
    let p = e.preview(&t.id).unwrap();
    assert!(p.operations.iter().all(|o| o.action == Action::Unchanged));
}

#[test]
fn parallel_crash_child() {
    let Ok(root) = std::env::var("FOLDERSYNC_PARALLEL_CRASH_FIXTURE") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("FolderSync-parallel-")
    );
    let mut e = Engine::open(&root.join("state")).unwrap();
    let t = e.tasks().unwrap().remove(0);
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
}

#[test]
fn killed_parallel_process_recovers_both_stages_and_retries() {
    use std::process::{Command, Stdio};
    let (d, e, mut t) = fixture(Mode::Copy, 2, 512);
    for i in 0..2 {
        fs::write(t.a.join(format!("f{i}")), vec![i as u8; 1024 * 1024]).unwrap();
    }
    drop(e);
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "parallel_crash_child", "--nocapture"])
        .env("FOLDERSYNC_PARALLEL_CRASH_FIXTURE", d.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let observed = loop {
        if active_stages(&t.b) == 2 {
            break true;
        }
        if start.elapsed() > Duration::from_secs(10) {
            break false;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let _ = child.kill();
    child.wait().unwrap();
    assert!(
        observed,
        "Child never reached two partial destination stages"
    );
    for i in 0..2 {
        assert!(!t.b.join(format!("f{i}")).exists());
    }
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    assert!(
        e.history(&t.id)
            .unwrap()
            .iter()
            .any(|p| p.status == "Interrupted")
    );
    assert_eq!(fs::read_dir(&t.b).unwrap().count(), 0);
    t.bandwidth_kib = 0;
    e.save_task(&t).unwrap();
    let p = e.preview(&t.id).unwrap();
    assert_eq!(e.apply(&p.id, false).unwrap().status, "Applied");
    for i in 0..2 {
        assert_eq!(
            fs::read(t.a.join(format!("f{i}"))).unwrap(),
            vec![i as u8; 1024 * 1024]
        );
        assert_eq!(
            fs::read(t.b.join(format!("f{i}"))).unwrap(),
            vec![i as u8; 1024 * 1024]
        );
    }
}
