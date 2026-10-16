use foldersync_engine::*;
use std::fs;
fn setup(mode: Mode) -> (tempfile::TempDir, Engine, Task) {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-test-")
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
        mode,
        ..Task::default()
    };
    e.save_task(&t).unwrap();
    (d, e, t)
}
fn run(e: &mut Engine, t: &Task) {
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
}
#[test]
fn two_way_preview_conflict_deletion_and_restart() {
    let (d, mut e, t) = setup(Mode::TwoWay);
    fs::write(t.a.join("one.txt"), "A").unwrap();
    fs::write(t.b.join("two.txt"), "B").unwrap();
    fs::create_dir(t.a.join("empty")).unwrap();
    let p = e.preview(&t.id).unwrap();
    assert!(!t.b.join("one.txt").exists());
    e.apply(&p.id, false).unwrap();
    assert_eq!(fs::read(t.b.join("one.txt")).unwrap(), b"A");
    assert!(t.b.join("empty").is_dir());
    fs::write(t.a.join("one.txt"), "edit A").unwrap();
    fs::write(t.b.join("two.txt"), "edit B").unwrap();
    run(&mut e, &t);
    assert_eq!(fs::read(t.a.join("two.txt")).unwrap(), b"edit B");
    fs::write(t.a.join("one.txt"), "conflict A").unwrap();
    fs::write(t.b.join("one.txt"), "conflict B").unwrap();
    let p = e.preview(&t.id).unwrap();
    assert!(p.operations.iter().any(|o| o.action == Action::Conflict));
    let p = e.resolve(&p.id, "one.txt", "a").unwrap();
    assert_eq!(fs::read(t.b.join("one.txt")).unwrap(), b"conflict B");
    e.apply(&p.id, true).unwrap();
    assert_eq!(fs::read(t.b.join("one.txt")).unwrap(), b"conflict A");
    fs::remove_file(t.a.join("two.txt")).unwrap();
    let p = e.preview(&t.id).unwrap();
    assert!(e.apply(&p.id, false).is_err());
    e.apply(&p.id, true).unwrap();
    assert!(!t.b.join("two.txt").exists());
    drop(e);
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    assert_eq!(e.tasks().unwrap().len(), 1);
    let p = e.preview(&t.id).unwrap();
    assert!(p.operations.iter().all(|o| o.action == Action::Unchanged));
}
#[test]
fn stale_and_offline_never_delete() {
    let (_d, mut e, t) = setup(Mode::TwoWay);
    fs::write(t.a.join("f"), "original").unwrap();
    let p = e.preview(&t.id).unwrap();
    fs::write(t.a.join("f"), "changed").unwrap();
    assert!(e.apply(&p.id, true).is_err());
    assert!(!t.b.join("f").exists());
    fs::remove_dir(&t.b).unwrap();
    assert!(e.preview(&t.id).is_err());
    assert_eq!(fs::read(t.a.join("f")).unwrap(), b"changed");
}
#[test]
fn modes_copy_mirror_move() {
    for mode in [Mode::Copy, Mode::Mirror, Mode::Move] {
        let (_d, mut e, t) = setup(mode);
        fs::write(t.a.join("source"), "content").unwrap();
        fs::write(t.b.join("extra"), "keep").unwrap();
        run(&mut e, &t);
        assert_eq!(fs::read(t.b.join("source")).unwrap(), b"content");
        assert_eq!(t.b.join("extra").exists(), mode != Mode::Mirror);
        assert_eq!(t.a.join("source").exists(), mode != Mode::Move);
        if mode == Mode::Copy {
            fs::remove_file(t.a.join("source")).unwrap();
            run(&mut e, &t);
            assert!(t.b.join("source").exists());
        }
    }
}
#[test]
fn delete_edit_preserves_survivor() {
    let (_d, mut e, t) = setup(Mode::TwoWay);
    fs::write(t.a.join("f"), "base").unwrap();
    run(&mut e, &t);
    fs::remove_file(t.a.join("f")).unwrap();
    fs::write(t.b.join("f"), "edited").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    assert_eq!(fs::read(t.b.join("f")).unwrap(), b"edited");
    assert!(!t.a.join("f").exists());
}
#[test]
fn folder_delete_edit_is_attention_instead_of_partial_directory_removal() {
    let (_d, mut e, t) = setup(Mode::TwoWay);
    fs::create_dir(t.a.join("folder")).unwrap();
    fs::write(t.a.join("folder/f"), "base").unwrap();
    run(&mut e, &t);
    fs::remove_dir_all(t.a.join("folder")).unwrap();
    fs::write(t.b.join("folder/f"), "edited").unwrap();
    let p = e.preview(&t.id).unwrap();
    let result = e.apply(&p.id, true).unwrap();
    assert_eq!(result.status, "Attention");
    assert_eq!(fs::read(t.b.join("folder/f")).unwrap(), b"edited");
}
