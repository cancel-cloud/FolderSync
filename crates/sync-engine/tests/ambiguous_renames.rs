use foldersync_engine::*;
use std::fs;

fn pair() -> (tempfile::TempDir, Engine, Task) {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-rename-ambiguity-")
        .tempdir()
        .unwrap();
    let t = Task {
        a: d.path().join("A"),
        b: d.path().join("B"),
        ..Task::default()
    };
    fs::create_dir(&t.a).unwrap();
    fs::create_dir(&t.b).unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    e.save_task(&t).unwrap();
    fs::write(t.a.join("old.txt"), b"common bytes").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    (d, e, t)
}

#[test]
fn opposing_renames_preserve_baseline_and_attention_while_independent_files_sync() {
    let (d, mut e, t) = pair();
    fs::rename(t.a.join("old.txt"), t.a.join("left.txt")).unwrap();
    fs::rename(t.b.join("old.txt"), t.b.join("right.txt")).unwrap();
    fs::write(t.a.join("independent.txt"), b"independent").unwrap();
    let p = e.preview(&t.id).unwrap();
    for path in ["old.txt", "left.txt", "right.txt"] {
        assert_eq!(
            p.operations.iter().find(|o| o.path == path).unwrap().action,
            Action::Conflict,
            "{path}"
        );
    }
    assert!(!t.b.join("left.txt").exists() && !t.a.join("right.txt").exists());
    let p = e.apply(&p.id, false).unwrap();
    assert_eq!(p.status, "Attention");
    assert_eq!(
        fs::read(t.b.join("independent.txt")).unwrap(),
        b"independent"
    );
    assert!(!t.b.join("left.txt").exists() && !t.a.join("right.txt").exists());
    drop(e);
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    let p = e.preview(&t.id).unwrap();
    assert_eq!(
        p.operations
            .iter()
            .filter(|o| o.action == Action::Conflict)
            .count(),
        3
    );
    // Deliberate per-path choices still only revise Preview.
    let p = e.resolve(&p.id, "left.txt", "a").unwrap();
    let p = e.resolve(&p.id, "right.txt", "b").unwrap();
    assert!(!t.b.join("left.txt").exists() && !t.a.join("right.txt").exists());
    assert!(e.apply(&p.id, false).is_err());
    e.apply(&p.id, true).unwrap();
    let p = e.preview(&t.id).unwrap();
    assert!(p.operations.iter().all(|o| o.action == Action::Unchanged));
    for root in [&t.a, &t.b] {
        for path in ["left.txt", "right.txt"] {
            assert_eq!(fs::read(root.join(path)).unwrap(), b"common bytes");
        }
    }
}

#[test]
fn matching_rename_on_both_sides_and_unrelated_duplicate_are_not_ambiguous() {
    let (_d, mut e, t) = pair();
    fs::write(t.a.join("duplicate.txt"), b"common bytes").unwrap();
    let p = e.preview(&t.id).unwrap();
    assert!(p.operations.iter().all(|o| o.action != Action::Conflict));
    e.apply(&p.id, false).unwrap();
    for root in [&t.a, &t.b] {
        fs::rename(root.join("old.txt"), root.join("same-name.txt")).unwrap();
    }
    let p = e.preview(&t.id).unwrap();
    assert!(p.operations.iter().all(|o| o.action == Action::Unchanged));
    e.apply(&p.id, false).unwrap();
}

#[test]
fn native_opposing_rename_notifications_reach_incremental_conflict_planning() {
    use std::{
        collections::BTreeSet,
        time::{Duration, Instant},
    };
    let (_d, mut e, t) = pair();
    let watcher = watch::WatchService::new(&t).unwrap();
    fs::rename(t.a.join("old.txt"), t.a.join("left.txt")).unwrap();
    fs::rename(t.b.join("old.txt"), t.b.join("right.txt")).unwrap();
    let mut paths = BTreeSet::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        paths.extend(watcher.poll().paths);
        if ["old.txt", "left.txt", "right.txt"]
            .iter()
            .all(|p| paths.contains(*p))
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        ["old.txt", "left.txt", "right.txt"]
            .iter()
            .all(|p| paths.contains(*p)),
        "{paths:?}"
    );
    e.queue_paths(&t.id, &paths.into_iter().collect::<Vec<_>>(), false)
        .unwrap();
    let p = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(p.status, "Attention");
    assert_eq!(p.metrics.reconciliations, 0);
    assert_eq!(
        p.operations
            .iter()
            .filter(|o| o.action == Action::Conflict)
            .count(),
        3
    );
    assert!(!t.a.join("right.txt").exists() && !t.b.join("left.txt").exists());
}

#[test]
fn coalesced_incremental_events_hold_ambiguous_creates_and_removals_even_with_delete_approval() {
    let (_d, mut e, mut t) = pair();
    t.deletion_policy = DeletionPolicy {
        enabled: true,
        max_entries: 20,
        max_percent: 100,
    };
    e.save_task(&t).unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    fs::rename(t.a.join("old.txt"), t.a.join("one.txt")).unwrap();
    fs::copy(t.a.join("one.txt"), t.a.join("two.txt")).unwrap();
    e.queue_paths(
        &t.id,
        &["old.txt".into(), "one.txt".into(), "two.txt".into()],
        false,
    )
    .unwrap();
    let p = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(p.status, "Attention");
    assert_eq!(p.metrics.reconciliations, 0);
    assert_eq!(p.operations.len(), 3);
    assert!(p.operations.iter().all(|o| o.action == Action::Conflict));
    assert_eq!(fs::read(t.b.join("old.txt")).unwrap(), b"common bytes");
    assert!(!t.b.join("one.txt").exists() && !t.b.join("two.txt").exists());
}

#[test]
fn identical_content_multiple_origins_never_guess_rename_pairing() {
    let (_d, mut e, t) = pair();
    fs::write(t.a.join("other.txt"), b"common bytes").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    fs::rename(t.a.join("old.txt"), t.a.join("one.txt")).unwrap();
    fs::rename(t.a.join("other.txt"), t.a.join("two.txt")).unwrap();
    let p = e.preview(&t.id).unwrap();
    assert_eq!(p.operations.len(), 4);
    assert!(p.operations.iter().all(|o| o.action == Action::Conflict));
    e.apply(&p.id, false).unwrap();
    assert!(t.b.join("old.txt").exists() && t.b.join("other.txt").exists());
    assert!(!t.b.join("one.txt").exists() && !t.b.join("two.txt").exists());
}
