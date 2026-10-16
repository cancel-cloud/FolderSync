use foldersync_engine::*;
use std::{
    fs,
    time::{Duration, Instant},
};
#[test]
fn destination_edit_during_staging_must_not_be_overwritten() {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-race-")
        .tempdir()
        .unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    let mut t = Task {
        a,
        b,
        mode: Mode::Copy,
        ..Task::default()
    };
    e.save_task(&t).unwrap();
    fs::write(t.a.join("f"), "base").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    t.bandwidth_kib = 512;
    e.save_task(&t).unwrap();
    fs::write(t.a.join("f"), vec![42; 1024 * 1024]).unwrap();
    let p = e.preview(&t.id).unwrap();
    let target = t.b.clone();
    let writer = std::thread::spawn(move || {
        let start = Instant::now();
        loop {
            if fs::read_dir(&target).unwrap().any(|e| {
                e.unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".foldersync-stage-")
            }) {
                fs::write(target.join("f"), "concurrent B edit").unwrap();
                return;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "No stage observed"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let result = e.apply(&p.id, true);
    writer.join().unwrap();
    assert!(
        result.is_err(),
        "A concurrent destination edit must invalidate replacement"
    );
    assert_eq!(fs::read(t.b.join("f")).unwrap(), b"concurrent B edit");
}
#[cfg(windows)]
#[test]
fn locked_move_destination_never_removes_source() {
    use std::os::windows::fs::OpenOptionsExt;
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
    fs::write(t.a.join("f"), "keep source").unwrap();
    fs::write(t.b.join("f"), "old").unwrap();
    let p = e.preview(&t.id).unwrap();
    let p = e.resolve(&p.id, "f", "a").unwrap();
    let _lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(t.b.join("f"))
        .unwrap();
    assert!(e.apply(&p.id, true).is_err());
    assert_eq!(fs::read(t.a.join("f")).unwrap(), b"keep source");
}
