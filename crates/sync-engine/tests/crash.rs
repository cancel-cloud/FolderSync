use foldersync_engine::*;
use std::{
    fs,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
#[test]
fn crash_child_process() {
    let Ok(root) = std::env::var("FOLDERSYNC_CRASH_FIXTURE") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("FolderSync-crash-")
    );
    let mut e = Engine::open(&root.join("state")).unwrap();
    let t = e.tasks().unwrap().remove(0);
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
}
#[test]
fn termination_midcopy_preserves_original_and_recovers_staging() {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-crash-")
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
        ..Task::default()
    };
    e.save_task(&t).unwrap();
    fs::write(t.a.join("f"), "original").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    t.bandwidth_kib = 256;
    e.save_task(&t).unwrap();
    fs::write(t.a.join("f"), vec![42; 1024 * 1024]).unwrap();
    drop(e);
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_child_process", "--nocapture"])
        .env("FOLDERSYNC_CRASH_FIXTURE", d.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let observed = loop {
        if fs::read_dir(&t.b).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".foldersync-stage-")
        }) {
            break true;
        }
        if start.elapsed() > Duration::from_secs(10) {
            break false;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let _ = child.kill();
    child.wait().unwrap();
    assert!(observed, "Child never reached staging");
    assert_eq!(fs::read(t.b.join("f")).unwrap(), b"original");
    assert_eq!(fs::metadata(t.a.join("f")).unwrap().len(), 1024 * 1024);
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    assert!(
        e.history(&t.id)
            .unwrap()
            .iter()
            .any(|p| p.status == "Interrupted")
    );
    assert!(
        !fs::read_dir(&t.b).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".foldersync-stage-")),
        "Owned staging files must be recovered"
    );
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    assert_eq!(
        fs::read(t.a.join("f")).unwrap(),
        fs::read(t.b.join("f")).unwrap()
    );
}
