use foldersync_engine::*;
use std::fs;

fn pair() -> (tempfile::TempDir, Engine, Task) {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-event-regression-")
        .tempdir()
        .unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let mut engine = Engine::open(&d.path().join("state")).unwrap();
    let task = Task {
        a,
        b,
        ..Task::default()
    };
    engine.save_task(&task).unwrap();
    fs::write(task.a.join("file.txt"), "base").unwrap();
    let plan = engine.preview(&task.id).unwrap();
    engine.apply(&plan.id, false).unwrap();
    (d, engine, task)
}

#[test]
fn event_invalidation_detects_same_length_timestamp_preserving_edit() {
    let (_d, mut engine, task) = pair();
    let path = task.a.join("file.txt");
    let before = filetime::FileTime::from_last_modification_time(&fs::metadata(&path).unwrap());
    fs::write(&path, "edit").unwrap();
    filetime::set_file_mtime(&path, before).unwrap();
    assert_eq!(
        filetime::FileTime::from_last_modification_time(&fs::metadata(&path).unwrap()),
        before
    );
    engine
        .queue_paths(&task.id, &["file.txt".into()], false)
        .unwrap();
    // Measure discovery separately: process_queue also includes Apply validation
    // and post-transfer verification in its cumulative counters.
    let paths = engine.queued(&task.id).unwrap();
    let plan = engine.preview_paths(&task.id, &paths).unwrap();
    assert_eq!(plan.metrics.reconciliations, 0);
    assert_eq!(plan.metrics.scanned_entries, 2);
    assert_eq!(plan.metrics.hashed_bytes, 8);
    engine.apply(&plan.id, false).unwrap();
    assert_eq!(fs::read(task.b.join("file.txt")).unwrap(), b"edit");
}

#[test]
fn delayed_duplicate_events_use_current_bytes_and_do_not_replay_old_edits() {
    let (_d, mut engine, task) = pair();
    fs::write(task.a.join("file.txt"), "first").unwrap();
    engine
        .queue_paths(&task.id, &["file.txt".into()], false)
        .unwrap();
    fs::write(task.a.join("file.txt"), "last").unwrap();
    engine
        .queue_paths(&task.id, &["file.txt".into(), "file.txt".into()], false)
        .unwrap();
    assert_eq!(engine.queued(&task.id).unwrap(), vec!["file.txt"]);
    engine.process_queue(&task.id).unwrap().unwrap();
    assert_eq!(fs::read(task.b.join("file.txt")).unwrap(), b"last");
    // An older native notification can arrive after the completed transfer.
    engine
        .queue_paths(&task.id, &["file.txt".into()], false)
        .unwrap();
    let repeated = engine.process_queue(&task.id).unwrap().unwrap();
    assert!(
        repeated
            .operations
            .iter()
            .all(|o| o.action == Action::Unchanged)
    );
    assert_eq!(fs::read(task.a.join("file.txt")).unwrap(), b"last");
    assert_eq!(fs::read(task.b.join("file.txt")).unwrap(), b"last");
}

#[test]
fn rename_events_preserve_old_destination_until_exact_plan_approval() {
    let (_d, mut engine, task) = pair();
    fs::rename(task.a.join("file.txt"), task.a.join("renamed.txt")).unwrap();
    engine
        .queue_paths(&task.id, &["renamed.txt".into(), "file.txt".into()], false)
        .unwrap();
    let reviewed = engine.process_queue(&task.id).unwrap().unwrap();
    assert_eq!(reviewed.status, "Review required");
    assert!(
        reviewed
            .operations
            .iter()
            .any(|o| o.path == "file.txt" && o.action == Action::DeleteB)
    );
    assert!(
        reviewed
            .operations
            .iter()
            .any(|o| o.path == "renamed.txt" && o.action == Action::CopyToB)
    );
    assert!(engine.apply(&reviewed.id, false).is_err());
    assert_eq!(fs::read(task.b.join("file.txt")).unwrap(), b"base");
    assert!(!task.b.join("renamed.txt").exists());
    engine.apply(&reviewed.id, true).unwrap();
    assert!(!task.b.join("file.txt").exists());
    assert_eq!(fs::read(task.b.join("renamed.txt")).unwrap(), b"base");
    let versions = engine.versions(&task.id).unwrap();
    assert!(
        versions
            .iter()
            .any(|v| v.path == "file.txt" && fs::read(&v.stored).unwrap() == b"base")
    );
    let repeated = engine.preview(&task.id).unwrap();
    assert!(
        repeated
            .operations
            .iter()
            .all(|o| o.action == Action::Unchanged)
    );
}
