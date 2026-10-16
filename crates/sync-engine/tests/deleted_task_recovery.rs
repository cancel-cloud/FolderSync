use foldersync_engine::*;
use std::fs;
fn fixture() -> (tempfile::TempDir, Engine, Task, Version) {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-deleted-recovery-")
        .tempdir()
        .unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    let t = Task {
        a,
        b,
        name: "Recover deleted configuration".into(),
        ..Task::default()
    };
    e.save_task(&t).unwrap();
    fs::write(t.a.join("f.txt"), "original").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    fs::write(t.a.join("f.txt"), "current").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    let v = e.versions(&t.id).unwrap().remove(0);
    (d, e, t, v)
}
#[test]
fn deleted_task_versions_remain_reviewable_and_restorable_without_reactivating_sync() {
    let (d, mut e, t, v) = fixture();
    let stale = e.restore_preview(&v.id).unwrap();
    e.delete_task(&t.id).unwrap();
    assert!(e.tasks().unwrap().is_empty());
    assert!(e.preview(&t.id).is_err());
    assert!(e.restore_apply(&stale.id).is_err());
    let archive = e.recovery_tasks().unwrap();
    assert_eq!(archive.len(), 1);
    assert_eq!(archive[0].task.name, t.name);
    assert!(
        !archive[0].task.monitor
            && !archive[0].task.login
            && !archive[0].task.deletion_policy.enabled
    );
    assert_eq!(fs::read(t.b.join("f.txt")).unwrap(), b"current");
    assert!(e.save_task(&t).is_err());
    drop(e);
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    let p = e.restore_preview(&v.id).unwrap();
    assert!(p.archived);
    assert_eq!(p.target_root, t.b);
    assert_eq!(fs::read(t.b.join("f.txt")).unwrap(), b"current");
    fs::write(t.b.join("f.txt"), "changed after preview").unwrap();
    assert!(e.restore_apply(&p.id).is_err());
    let p = e.restore_preview(&v.id).unwrap();
    e.restore_apply(&p.id).unwrap();
    assert_eq!(fs::read(t.b.join("f.txt")).unwrap(), b"original");
    assert!(
        e.versions(&t.id)
            .unwrap()
            .iter()
            .any(|v| fs::read(&v.stored).unwrap() == b"changed after preview")
    );
    assert!(e.tasks().unwrap().is_empty());
    assert!(e.restore_apply(&p.id).is_err());
    let db = rusqlite::Connection::open(d.path().join("state/foldersync.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM baseline WHERE task=?", [&t.id], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM dirty WHERE task=?", [&t.id], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn archived_restore_rejects_replaced_root_but_does_not_require_other_side_online() {
    let (d, mut e, t, v) = fixture();
    e.delete_task(&t.id).unwrap();
    fs::rename(&t.b, d.path().join("B-original")).unwrap();
    fs::create_dir(&t.b).unwrap();
    assert!(e.restore_preview(&v.id).is_err());
    assert_eq!(fs::read_dir(&t.b).unwrap().count(), 0);
    fs::remove_dir(&t.b).unwrap();
    fs::rename(d.path().join("B-original"), &t.b).unwrap();
    fs::rename(&t.a, d.path().join("A-offline")).unwrap();
    let p = e.restore_preview(&v.id).unwrap();
    e.restore_apply(&p.id).unwrap();
    assert_eq!(fs::read(t.b.join("f.txt")).unwrap(), b"original");
    assert!(e.tasks().unwrap().is_empty());
}
#[test]
fn legacy_orphan_catalog_is_recovered_from_recorded_run_roots() {
    let (d, mut e, t, v) = fixture();
    e.delete_task(&t.id).unwrap();
    drop(e);
    let db = rusqlite::Connection::open(d.path().join("state/foldersync.sqlite3")).unwrap();
    db.execute_batch("DELETE FROM recovery_tasks; PRAGMA user_version=3;")
        .unwrap();
    drop(db);
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    assert_eq!(e.recovery_tasks().unwrap()[0].task.id, t.id);
    let p = e.restore_preview(&v.id).unwrap();
    e.restore_apply(&p.id).unwrap();
    assert_eq!(fs::read(t.b.join("f.txt")).unwrap(), b"original");
}

#[test]
fn archived_task_staging_is_cleaned_at_restart_without_touching_user_files() {
    let (d, mut e, t, _) = fixture();
    let stage = t.b.join(".foldersync-stage-interrupted-recovery");
    fs::write(&stage, "incomplete owned transfer").unwrap();
    let db = rusqlite::Connection::open(d.path().join("state/foldersync.sqlite3")).unwrap();
    db.execute(
        "INSERT INTO staging VALUES(?,?)",
        rusqlite::params![stage.to_str().unwrap(), t.id],
    )
    .unwrap();
    e.delete_task(&t.id).unwrap();
    drop(e);
    let e = Engine::open(&d.path().join("state")).unwrap();
    assert!(!stage.exists());
    assert_eq!(fs::read(t.b.join("f.txt")).unwrap(), b"current");
    assert!(e.tasks().unwrap().is_empty());
    assert_eq!(e.recovery_tasks().unwrap().len(), 1);
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM staging", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}
