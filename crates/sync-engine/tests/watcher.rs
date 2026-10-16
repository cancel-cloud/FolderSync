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

#[test]
fn native_save_burst_is_coalesced_while_consumer_is_busy() {
    let dir = tempfile::Builder::new()
        .prefix("FolderSync-busy-watch-")
        .tempdir()
        .unwrap();
    let task = Task {
        a: dir.path().join("A"),
        b: dir.path().join("B"),
        ..Task::default()
    };
    fs::create_dir(&task.a).unwrap();
    fs::create_dir(&task.b).unwrap();
    let watch = WatchService::new(&task).unwrap();
    // Let the native callback run, but do not poll while the engine is busy.
    for index in 0..2500 {
        fs::write(task.a.join("busy.txt"), index.to_string()).unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    std::thread::sleep(Duration::from_millis(100));
    let batch = watch.poll();
    assert!(
        !batch.uncertain,
        "A repeated save burst must not overflow the application's raw event queue: {batch:?}"
    );
    assert_eq!(batch.paths, vec!["busy.txt"]);
    assert!(
        batch.events > 2048,
        "The regression must exercise more than the old queue capacity"
    );
    fs::write(task.b.join("later.txt"), "arrived after drain").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if watch.poll().paths.contains(&"later.txt".to_string()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("Event after draining the collector was lost");
}
