use foldersync_engine::*;
use std::fs;
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
