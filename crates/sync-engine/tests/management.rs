use foldersync_engine::*;
use std::fs;
#[test]
fn bulk_group_assignment_is_atomic_persistent_and_invalidates_old_reviews() {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-groups-")
        .tempdir()
        .unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let state = d.path().join("state");
    let mut e = Engine::open(&state).unwrap();
    let t = Task {
        a,
        b,
        group: "Original".into(),
        ..Task::default()
    };
    e.save_task(&t).unwrap();
    fs::write(t.a.join("proof.txt"), "only A").unwrap();
    let preview = e.preview(&t.id).unwrap();
    assert!(
        e.assign_group(&[t.id.clone(), uuid::Uuid::new_v4().to_string()], "Partial")
            .is_err()
    );
    assert_eq!(e.tasks().unwrap()[0].group, "Original");
    e.assign_group(&[t.id.clone(), t.id.clone()], "  Engineering  ")
        .unwrap();
    assert_eq!(e.tasks().unwrap()[0].group, "Engineering");
    assert!(
        e.apply(&preview.id, false).is_err(),
        "Configuration changes invalidate reviewed task snapshot"
    );
    assert!(!t.b.join("proof.txt").exists());
    assert_eq!(fs::read(t.a.join("proof.txt")).unwrap(), b"only A");
    drop(e);
    let mut e = Engine::open(&state).unwrap();
    assert_eq!(e.tasks().unwrap()[0].group, "Engineering");
    e.assign_group(std::slice::from_ref(&t.id), "").unwrap();
    assert_eq!(e.tasks().unwrap()[0].group, "");
}
#[test]
fn legacy_index_migration_preserves_task_configuration() {
    let d = tempfile::tempdir().unwrap();
    let db = rusqlite::Connection::open(d.path().join("foldersync.sqlite3")).unwrap();
    db.execute_batch("CREATE TABLE tasks(id TEXT PRIMARY KEY,json TEXT NOT NULL,root_a TEXT NOT NULL,root_b TEXT NOT NULL,generation INTEGER NOT NULL DEFAULT 0); CREATE TABLE entries(task TEXT NOT NULL,side TEXT NOT NULL,path TEXT NOT NULL,parent TEXT NOT NULL,json TEXT NOT NULL,digest TEXT NOT NULL,PRIMARY KEY(task,side,path)); PRAGMA user_version=1;").unwrap();
    let t = Task {
        name: "Legacy preserved".into(),
        ..Task::default()
    };
    db.execute(
        "INSERT INTO tasks(id,json,root_a,root_b) VALUES(?,?,?,?)",
        rusqlite::params![t.id, serde_json::to_string(&t).unwrap(), "a", "b"],
    )
    .unwrap();
    drop(db);
    let e = Engine::open(d.path()).unwrap();
    assert_eq!(e.tasks().unwrap()[0].name, "Legacy preserved");
    let db = rusqlite::Connection::open(d.path().join("foldersync.sqlite3")).unwrap();
    let version: i64 = db
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 2);
}
#[test]
fn import_is_atomic_and_disables_triggers() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    let t = Task {
        a,
        b,
        monitor: true,
        login: true,
        ..Task::default()
    };
    let mut bad = t.clone();
    bad.name = String::new();
    let input = serde_json::json!({"schema":1,"tasks":[t,bad]});
    assert!(e.import(&input.to_string()).is_err());
    assert!(
        e.tasks().unwrap().is_empty(),
        "Invalid batch must not leave partial imports"
    );
    let input = serde_json::json!({"schema":1,"tasks":[t]});
    assert_eq!(e.import(&input.to_string()).unwrap(), 1);
    let imported = e.tasks().unwrap().remove(0);
    assert!(!imported.monitor && !imported.login && imported.schedule.kind.is_empty());
    assert_ne!(imported.id, t.id);
    assert!(e.export().unwrap().contains("schema"));
}
#[test]
fn refuses_future_database_schema() {
    let d = tempfile::tempdir().unwrap();
    drop(Engine::open(d.path()).unwrap());
    let db = rusqlite::Connection::open(d.path().join("foldersync.sqlite3")).unwrap();
    db.execute_batch("PRAGMA user_version=999").unwrap();
    drop(db);
    assert!(
        Engine::open(d.path()).is_err(),
        "A future schema must never be silently downgraded"
    );
}

#[test]
fn login_only_tasks_are_guarded_against_intersecting_pairs() {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-login-overlap-")
        .tempdir()
        .unwrap();
    for name in ["A", "B", "C"] {
        fs::create_dir(d.path().join(name)).unwrap();
    }
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    let mut first = Task {
        a: d.path().join("A"),
        b: d.path().join("B"),
        login: true,
        ..Task::default()
    };
    e.save_task(&first).unwrap();
    let mut second = Task {
        a: d.path().join("B"),
        b: d.path().join("C"),
        ..Task::default()
    };
    assert!(
        e.save_task(&second).is_err(),
        "Existing login task must protect its roots"
    );
    first.login = false;
    e.save_task(&first).unwrap();
    second.login = true;
    assert!(
        e.save_task(&second).is_err(),
        "Enabling login must validate other task roots"
    );
    assert_eq!(e.tasks().unwrap().len(), 1);
}
