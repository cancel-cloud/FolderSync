use foldersync_engine::*;
use std::fs;
#[test]
fn unicode_and_long_paths_transfer_with_empty_folders() {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-long-")
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
        ..Task::default()
    };
    e.save_task(&t).unwrap();
    let relative = format!(
        "{}/{}/{}/{}/{}/Konstruktion-ü-東京.txt",
        "a".repeat(48),
        "b".repeat(48),
        "c".repeat(48),
        "d".repeat(48),
        "e".repeat(48)
    );
    fs::create_dir_all(t.a.join(&relative).parent().unwrap()).unwrap();
    fs::write(t.a.join(&relative), "unicode data").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    assert_eq!(fs::read(t.b.join(relative)).unwrap(), b"unicode data");
}
#[test]
fn nested_move_verifies_every_file_before_source_directory_removal() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    let t = Task {
        a,
        b,
        mode: Mode::Move,
        ..Task::default()
    };
    e.save_task(&t).unwrap();
    fs::create_dir_all(t.a.join("dir/empty")).unwrap();
    fs::write(t.a.join("dir/f"), "move verified").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    assert!(!t.a.join("dir").exists());
    assert!(t.b.join("dir/empty").is_dir());
    assert_eq!(fs::read(t.b.join("dir/f")).unwrap(), b"move verified");
}
