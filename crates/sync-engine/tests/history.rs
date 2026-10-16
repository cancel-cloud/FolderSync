use foldersync_engine::*;
use std::fs;
#[test]
fn history_is_bounded_without_pruning_interrupted_recovery_or_baselines() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let state = d.path().join("state");
    let mut e = Engine::open(&state).unwrap();
    let t = Task {
        a,
        b,
        ..Task::default()
    };
    e.save_task(&t).unwrap();
    fs::write(t.a.join("baseline-file"), "base").unwrap();
    let initial = e.preview(&t.id).unwrap();
    e.apply(&initial.id, true).unwrap();
    let p = e.preview(&t.id).unwrap();
    let db = rusqlite::Connection::open(state.join("foldersync.sqlite3")).unwrap();
    db.execute("UPDATE runs SET status='Interrupted' WHERE id=?", [p.id])
        .unwrap();
    for _ in 0..105 {
        e.preview(&t.id).unwrap();
    }
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM runs WHERE task=?", [&t.id], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 101);
    let interrupted: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE status='Interrupted'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(interrupted, 1);
    let baselines: i64 = db
        .query_row("SELECT COUNT(*) FROM baseline WHERE task=?", [&t.id], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(baselines, 1);
    assert_eq!(fs::read(t.b.join("baseline-file")).unwrap(), b"base");
}
