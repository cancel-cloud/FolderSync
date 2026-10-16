use foldersync_engine::*;
use std::fs;

fn fixture() -> (tempfile::TempDir, Engine, Task) {
    let d = tempfile::tempdir().unwrap();
    let t = Task {
        a: d.path().join("a"),
        b: d.path().join("b"),
        history_cap: 2,
        ..Task::default()
    };
    fs::create_dir(&t.a).unwrap();
    fs::create_dir(&t.b).unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    e.save_task(&t).unwrap();
    for text in ["first", "second"] {
        fs::write(t.a.join("f"), text).unwrap();
        let p = e.preview(&t.id).unwrap();
        e.apply(&p.id, true).unwrap();
    }
    (d, e, t)
}

#[test]
fn new_protected_versions_are_grouped_by_task_and_restore() {
    let (_d, mut e, t) = fixture();
    let v = e.versions(&t.id).unwrap().remove(0);
    assert_eq!(
        v.stored,
        e.storage().join("versions").join(&t.id).join(&v.id)
    );
    let p = e.restore_preview(&v.id).unwrap();
    e.restore_apply(&p.id).unwrap();
    assert_eq!(fs::read(t.b.join("f")).unwrap(), b"first");
}

#[test]
fn legacy_flat_versions_remain_restorable_and_obey_retention() {
    let (_d, mut e, t) = fixture();
    let mut v = e.versions(&t.id).unwrap().remove(0);
    let flat = e.storage().join("versions").join(&v.id);
    if v.stored != flat {
        fs::rename(&v.stored, &flat).unwrap();
    }
    v.stored = flat.clone();
    let db = rusqlite::Connection::open(e.storage().join("foldersync.sqlite3")).unwrap();
    db.execute(
        "UPDATE versions SET json=? WHERE id=?",
        rusqlite::params![serde_json::to_string(&v).unwrap(), v.id],
    )
    .unwrap();
    let p = e.restore_preview(&v.id).unwrap();
    e.restore_apply(&p.id).unwrap();
    assert_eq!(fs::read(t.b.join("f")).unwrap(), b"first");
    fs::write(t.a.join("f"), "first").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    for text in ["third", "fourth", "fifth"] {
        fs::write(t.a.join("f"), text).unwrap();
        let p = e.preview(&t.id).unwrap();
        e.apply(&p.id, true).unwrap();
    }
    assert!(!flat.exists());
    assert_eq!(e.versions(&t.id).unwrap().len(), 2);
}
