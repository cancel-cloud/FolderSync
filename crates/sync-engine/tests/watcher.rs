use foldersync_engine::{watch::WatchService, *};
use std::{
    fs,
    time::{Duration, Instant},
};
#[test]
fn native_notifications_observe_both_roots() {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-watch-")
        .tempdir()
        .unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let t = Task {
        a,
        b,
        ..Task::default()
    };
    let w = WatchService::new(&t).unwrap();
    fs::write(t.a.join("a.txt"), "A").unwrap();
    fs::write(t.b.join("b.txt"), "B").unwrap();
    let start = Instant::now();
    let mut paths = std::collections::BTreeSet::new();
    while start.elapsed() < Duration::from_secs(10) {
        paths.extend(w.poll().paths);
        if paths.contains("a.txt") && paths.contains("b.txt") {
            return;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    panic!("Missing native notifications: {paths:?}");
}
