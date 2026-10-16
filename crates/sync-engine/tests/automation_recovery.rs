use foldersync_engine::*;
use std::fs;

fn fixture() -> (tempfile::TempDir, Engine, Task) {
    let dir = tempfile::Builder::new()
        .prefix("FolderSync-automation-recovery-")
        .tempdir()
        .unwrap();
    let task = Task {
        a: dir.path().join("A"),
        b: dir.path().join("B"),
        ..Task::default()
    };
    fs::create_dir(&task.a).unwrap();
    fs::create_dir(&task.b).unwrap();
    let mut engine = Engine::open(&dir.path().join("state")).unwrap();
    engine.save_task(&task).unwrap();
    (dir, engine, task)
}

#[test]
fn unchanged_automatic_reconciliation_establishes_deletion_baseline() {
    let (_dir, mut engine, task) = fixture();
    fs::write(task.a.join("f"), "equal existing files").unwrap();
    fs::write(task.b.join("f"), "equal existing files").unwrap();
    engine.queue_paths(&task.id, &[], true).unwrap();
    engine.process_queue(&task.id).unwrap();
    fs::remove_file(task.a.join("f")).unwrap();
    let plan = engine.preview(&task.id).unwrap();
    assert_eq!(plan.operations[0].action, Action::DeleteB);
}

#[test]
fn automatic_equal_observations_are_hashed_once_per_side() {
    let (_dir, mut engine, task) = fixture();
    let bytes = b"verified equal content";
    fs::write(task.a.join("f"), bytes).unwrap();
    fs::write(task.b.join("f"), bytes).unwrap();
    engine.queue_paths(&task.id, &[], true).unwrap();
    let plan = engine.process_queue(&task.id).unwrap().unwrap();
    assert_eq!(plan.status, "Applied");
    assert_eq!(plan.metrics.hashed_bytes, (bytes.len() * 2) as u64);
    assert_eq!(plan.metrics.copied_bytes, 0);
    assert_eq!(fs::read(task.a.join("f")).unwrap(), bytes);
    assert_eq!(fs::read(task.b.join("f")).unwrap(), bytes);
}

#[test]
fn equal_external_resolution_advances_baseline_and_clears_conflict() {
    let (_dir, mut engine, task) = fixture();
    fs::write(task.a.join("f"), "base").unwrap();
    engine.queue_paths(&task.id, &[], true).unwrap();
    engine.process_queue(&task.id).unwrap();
    fs::write(task.a.join("f"), "A edit").unwrap();
    fs::write(task.b.join("f"), "B edit").unwrap();
    engine.queue_paths(&task.id, &["f".into()], false).unwrap();
    engine.process_queue(&task.id).unwrap();
    assert_eq!(engine.pending_conflicts(&task.id).unwrap(), 1);
    fs::write(task.a.join("f"), "common resolution").unwrap();
    fs::write(task.b.join("f"), "common resolution").unwrap();
    engine.queue_paths(&task.id, &["f".into()], false).unwrap();
    engine.process_queue(&task.id).unwrap();
    assert_eq!(engine.pending_conflicts(&task.id).unwrap(), 0);
    fs::remove_file(task.a.join("f")).unwrap();
    assert_eq!(
        engine.preview(&task.id).unwrap().operations[0].action,
        Action::DeleteB
    );
}

#[test]
fn interrupted_plan_cannot_be_revised_or_reapplied() {
    let (dir, mut engine, task) = fixture();
    fs::write(task.a.join("f"), "A original").unwrap();
    fs::write(task.b.join("f"), "B original").unwrap();
    let plan = engine.preview(&task.id).unwrap();
    let db = rusqlite::Connection::open(dir.path().join("state/foldersync.sqlite3")).unwrap();
    db.execute("UPDATE runs SET status='Applying' WHERE id=?", [&plan.id])
        .unwrap();
    drop(db);
    drop(engine);
    let mut engine = Engine::open(&dir.path().join("state")).unwrap();
    assert_eq!(engine.history(&task.id).unwrap()[0].status, "Interrupted");
    assert!(
        engine.resolve(&plan.id, "f", "a").is_err(),
        "Interrupted plans must not become revised previews"
    );
    assert!(
        engine.apply(&plan.id, true).is_err(),
        "Interrupted plans must require a fresh preview"
    );
    assert_eq!(fs::read(task.a.join("f")).unwrap(), b"A original");
    assert_eq!(fs::read(task.b.join("f")).unwrap(), b"B original");
}

#[test]
fn destructive_background_review_remains_explicitly_applyable() {
    let (_dir, mut engine, task) = fixture();
    fs::write(task.a.join("f"), "base").unwrap();
    engine.queue_paths(&task.id, &[], true).unwrap();
    engine.process_queue(&task.id).unwrap();
    fs::remove_file(task.a.join("f")).unwrap();
    engine.queue_paths(&task.id, &["f".into()], false).unwrap();
    let plan = engine.process_queue(&task.id).unwrap().unwrap();
    assert_eq!(plan.status, "Review required");
    assert!(engine.apply(&plan.id, false).is_err());
    engine.apply(&plan.id, true).unwrap();
    assert!(!task.b.join("f").exists());
}
