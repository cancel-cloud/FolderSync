use foldersync_engine::*;
use std::fs;
#[test]
fn restore_is_reviewed_and_stale_checked() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    let t = Task {
        a,
        b,
        ..Task::default()
    };
    e.save_task(&t).unwrap();
    fs::write(t.a.join("f"), "original").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    fs::write(t.a.join("f"), "updated").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    let v = e.versions(&t.id).unwrap().remove(0);
    let r = e.restore_preview(&v.id).unwrap();
    assert_eq!(fs::read(t.b.join("f")).unwrap(), b"updated");
    fs::write(t.b.join("f"), "newer edit").unwrap();
    assert!(e.restore_apply(&r.id).is_err());
    let r = e.restore_preview(&v.id).unwrap();
    e.restore_apply(&r.id).unwrap();
    assert_eq!(fs::read(t.b.join("f")).unwrap(), b"original");
    assert!(e.versions(&t.id).unwrap().len() >= 2);
}
