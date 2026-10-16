use foldersync_engine::*;
use std::fs;
#[test]
fn empty_directory_merkle_digest_is_current_after_recreation() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    fs::create_dir(a.join("empty")).unwrap();
    let state = d.path().join("state");
    let mut e = Engine::open(&state).unwrap();
    let t = Task {
        a,
        b,
        ..Task::default()
    };
    e.save_task(&t).unwrap();
    e.preview(&t.id).unwrap();
    let db = rusqlite::Connection::open(state.join("foldersync.sqlite3")).unwrap();
    let digest: String = db
        .query_row(
            "SELECT digest FROM merkle WHERE task=? AND side='A' AND path='empty'",
            [&t.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(digest, blake3::hash(b"").to_hex().to_string());
    fs::write(t.a.join("empty/f"), "value").unwrap();
    e.preview_paths(&t.id, &["empty/f".into()]).unwrap();
    let populated: String = db
        .query_row(
            "SELECT digest FROM merkle WHERE task=? AND side='A' AND path='empty'",
            [&t.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_ne!(digest, populated);
    fs::remove_file(t.a.join("empty/f")).unwrap();
    e.preview_paths(&t.id, &["empty/f".into()]).unwrap();
    let empty: String = db
        .query_row(
            "SELECT digest FROM merkle WHERE task=? AND side='A' AND path='empty'",
            [&t.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(empty, digest);
}
