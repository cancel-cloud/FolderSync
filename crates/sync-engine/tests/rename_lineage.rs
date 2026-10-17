#![cfg(windows)]
use foldersync_engine::*;
use std::fs;

fn fixture() -> (tempfile::TempDir, Engine, Task) {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-lineage-")
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
    fs::write(t.a.join("old.txt"), b"common").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    (d, e, t)
}
fn conflict(p: &Plan, path: &str) {
    assert_eq!(
        p.operations.iter().find(|o| o.path == path).unwrap().action,
        Action::Conflict,
        "{path}"
    );
}

// Without a baseline identity snapshot, a changed rename target is copied as
// an unrelated creation while the original's opposing edit remains separate.
#[test]
fn rename_then_edit_vs_opposing_edit_holds_both_paths_from_target_only_event() {
    let (_d, mut e, t) = fixture();
    fs::rename(t.a.join("old.txt"), t.a.join("renamed.txt")).unwrap();
    fs::write(t.a.join("renamed.txt"), b"renamed and edited on A").unwrap();
    fs::write(t.b.join("old.txt"), b"edited on B").unwrap();
    e.queue_paths(&t.id, &["renamed.txt".into()], false)
        .unwrap();
    let p = e.process_queue(&t.id).unwrap().unwrap();
    conflict(&p, "old.txt");
    conflict(&p, "renamed.txt");
    assert_eq!(p.metrics.reconciliations, 0);
    assert!(!t.b.join("renamed.txt").exists() && !t.a.join("old.txt").exists());
    assert_eq!(
        fs::read(t.a.join("renamed.txt")).unwrap(),
        b"renamed and edited on A"
    );
    assert_eq!(fs::read(t.b.join("old.txt")).unwrap(), b"edited on B");
}

// Splitting opposing notifications must not establish an unrelated create
// baseline before its delayed counterpart is observed, including after restart.
#[test]
fn split_opposing_targets_survive_restart_without_copying_either_original() {
    let (d, mut e, t) = fixture();
    fs::rename(t.a.join("old.txt"), t.a.join("left.txt")).unwrap();
    fs::rename(t.b.join("old.txt"), t.b.join("right.txt")).unwrap();
    fs::write(t.a.join("left.txt"), b"left edit").unwrap();
    fs::write(t.b.join("right.txt"), b"right edit").unwrap();
    e.queue_paths(&t.id, &["left.txt".into()], false).unwrap();
    let p = e.process_queue(&t.id).unwrap().unwrap();
    conflict(&p, "old.txt");
    conflict(&p, "left.txt");
    assert!(!t.b.join("left.txt").exists());
    drop(e);
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    e.queue_paths(&t.id, &["right.txt".into()], false).unwrap();
    let p = e.process_queue(&t.id).unwrap().unwrap();
    for name in ["old.txt", "left.txt", "right.txt"] {
        conflict(&p, name);
    }
    assert!(p.metrics.scanned_entries <= 6 && p.metrics.reconciliations == 0);
    assert!(!t.a.join("right.txt").exists() && !t.b.join("left.txt").exists());
    fs::write(t.a.join("independent"), b"independent").unwrap();
    e.queue_paths(&t.id, &["independent".into()], false)
        .unwrap();
    e.process_queue(&t.id).unwrap();
    assert_eq!(fs::read(t.b.join("independent")).unwrap(), b"independent");
    assert_eq!(e.pending_conflicts(&t.id).unwrap(), 3);
}

#[test]
fn old_name_events_before_targets_do_not_erase_persistent_rename_identity() {
    let (d, mut e, t) = fixture();
    fs::rename(t.a.join("old.txt"), t.a.join("left.txt")).unwrap();
    fs::rename(t.b.join("old.txt"), t.b.join("right.txt")).unwrap();
    e.queue_paths(&t.id, &["old.txt".into()], false).unwrap();
    e.process_queue(&t.id).unwrap();
    drop(e);
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    for target in ["left.txt", "right.txt"] {
        e.queue_paths(&t.id, &[target.into()], false).unwrap();
        let p = e.process_queue(&t.id).unwrap().unwrap();
        conflict(&p, "old.txt");
        conflict(&p, target);
    }
    assert!(!t.a.join("right.txt").exists() && !t.b.join("left.txt").exists());
}

#[test]
fn a_later_identical_copy_joins_the_unresolved_rename_group() {
    let (_d, mut e, t) = fixture();
    fs::rename(t.a.join("old.txt"), t.a.join("one.txt")).unwrap();
    fs::copy(t.a.join("one.txt"), t.a.join("two.txt")).unwrap();
    e.queue_paths(&t.id, &["one.txt".into()], false).unwrap();
    assert_eq!(
        e.process_queue(&t.id).unwrap().unwrap().status,
        "Review required"
    );
    e.queue_paths(&t.id, &["two.txt".into()], false).unwrap();
    let p = e.process_queue(&t.id).unwrap().unwrap();
    for path in ["old.txt", "one.txt", "two.txt"] {
        conflict(&p, path);
    }
    assert!(!t.b.join("one.txt").exists() && !t.b.join("two.txt").exists());
}

#[test]
fn native_rename_pair_survives_a_subsequent_atomic_editor_replacement() {
    use std::time::{Duration, Instant};
    let (_d, mut e, t) = fixture();
    let watcher = watch::WatchService::new(&t).unwrap();
    fs::rename(t.a.join("old.txt"), t.a.join("renamed.txt")).unwrap();
    fs::rename(t.a.join("renamed.txt"), t.a.join("editor-backup")).unwrap();
    fs::write(t.a.join("renamed.txt"), b"atomic editor replacement").unwrap();
    fs::remove_file(t.a.join("editor-backup")).unwrap();
    fs::write(t.b.join("old.txt"), b"opposing edit").unwrap();
    let start = Instant::now();
    let mut seen = std::collections::BTreeSet::new();
    while start.elapsed() < Duration::from_secs(10) {
        let batch = watcher.poll();
        seen.extend(batch.paths.iter().cloned());
        e.queue_watch_batch(&t.id, &batch).unwrap();
        if seen.contains("old.txt")
            && seen.contains("renamed.txt")
            && seen.contains("editor-backup")
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(seen.contains("old.txt") && seen.contains("renamed.txt"));
    let p = e.process_queue(&t.id).unwrap().unwrap();
    conflict(&p, "old.txt");
    conflict(&p, "renamed.txt");
    assert!(!t.b.join("renamed.txt").exists() && !t.a.join("old.txt").exists());
    assert_eq!(
        fs::read(t.a.join("renamed.txt")).unwrap(),
        b"atomic editor replacement"
    );
    assert_eq!(fs::read(t.b.join("old.txt")).unwrap(), b"opposing edit");
}

#[test]
fn reviewed_split_rename_choices_retire_old_identity_without_future_phantom_conflicts() {
    let (d, mut e, t) = fixture();
    fs::rename(t.a.join("old.txt"), t.a.join("left.txt")).unwrap();
    fs::rename(t.b.join("old.txt"), t.b.join("right.txt")).unwrap();
    fs::write(t.a.join("left.txt"), b"left edit").unwrap();
    let p = e.preview(&t.id).unwrap();
    for path in ["old.txt", "left.txt", "right.txt"] {
        conflict(&p, path);
    }
    let p = e.resolve(&p.id, "left.txt", "a").unwrap();
    let p = e.resolve(&p.id, "right.txt", "b").unwrap();
    let p = e.resolve(&p.id, "old.txt", "ignore").unwrap();
    assert!(!t.a.join("right.txt").exists() && !t.b.join("left.txt").exists());
    e.apply(&p.id, true).unwrap();
    let p = e.preview(&t.id).unwrap();
    assert!(p.operations.iter().all(|o| o.action == Action::Unchanged));
    e.apply(&p.id, false).unwrap();
    drop(e);
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    fs::write(t.a.join("left.txt"), b"later edit").unwrap();
    let p = e.preview_paths(&t.id, &["left.txt".into()]).unwrap();
    assert_eq!(p.operations.len(), 1);
    assert_eq!(p.operations[0].action, Action::CopyToB);
    e.apply(&p.id, false).unwrap();
    assert_eq!(fs::read(t.b.join("left.txt")).unwrap(), b"later edit");
}

#[test]
fn excluding_all_rename_targets_clears_attention_without_touching_excluded_originals() {
    let (_d, mut e, mut t) = fixture();
    fs::rename(t.a.join("old.txt"), t.a.join("left.txt")).unwrap();
    fs::rename(t.b.join("old.txt"), t.b.join("right.txt")).unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    assert_eq!(e.pending_conflicts(&t.id).unwrap(), 3);
    t.scope.exclude = vec!["left.txt".into(), "right.txt".into()];
    e.save_task(&t).unwrap();
    let p = e.preview(&t.id).unwrap();
    assert!(p.operations.iter().all(|o| o.action == Action::Unchanged));
    e.apply(&p.id, false).unwrap();
    assert_eq!(e.pending_conflicts(&t.id).unwrap(), 0);
    assert_eq!(fs::read(t.a.join("left.txt")).unwrap(), b"common");
    assert_eq!(fs::read(t.b.join("right.txt")).unwrap(), b"common");
}

#[test]
fn a_normal_atomic_editor_save_is_an_edit_without_a_phantom_backup_conflict() {
    use std::time::{Duration, Instant};
    let (_d, mut e, t) = fixture();
    let watcher = watch::WatchService::new(&t).unwrap();
    fs::rename(t.a.join("old.txt"), t.a.join("editor-backup")).unwrap();
    fs::write(t.a.join("old.txt"), b"ordinary atomic save").unwrap();
    fs::remove_file(t.a.join("editor-backup")).unwrap();
    let start = Instant::now();
    let mut seen = std::collections::BTreeSet::new();
    while start.elapsed() < Duration::from_secs(10) {
        let batch = watcher.poll();
        seen.extend(batch.paths.iter().cloned());
        e.queue_watch_batch(&t.id, &batch).unwrap();
        if seen.contains("old.txt") && seen.contains("editor-backup") {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(seen.contains("old.txt") && seen.contains("editor-backup"));
    let p = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(p.status, "Applied");
    assert_eq!(
        fs::read(t.b.join("old.txt")).unwrap(),
        b"ordinary atomic save"
    );
    assert_eq!(e.pending_conflicts(&t.id).unwrap(), 0);
}

#[test]
fn version_four_migration_retains_rename_evidence_and_invalidates_older_reviews() {
    let (d, mut e, t) = fixture();
    let old = e.preview(&t.id).unwrap();
    drop(e);
    let db = rusqlite::Connection::open(d.path().join("state/foldersync.sqlite3")).unwrap();
    db.execute_batch(
        "DROP TABLE baseline_identity; DROP TABLE rename_links; PRAGMA user_version=4;",
    )
    .unwrap();
    drop(db);
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    assert!(
        e.apply(&old.id, false).is_err(),
        "Pre-lineage reviews must be invalidated by migration"
    );
    fs::rename(t.a.join("old.txt"), t.a.join("renamed.txt")).unwrap();
    fs::write(t.a.join("renamed.txt"), b"new A").unwrap();
    fs::write(t.b.join("old.txt"), b"new B").unwrap();
    let p = e.preview_paths(&t.id, &["renamed.txt".into()]).unwrap();
    conflict(&p, "old.txt");
    conflict(&p, "renamed.txt");
}

#[test]
fn a_late_native_rename_hint_invalidates_an_older_independent_create_preview() {
    let (_d, mut e, t) = fixture();
    fs::rename(t.a.join("old.txt"), t.a.join("renamed.txt")).unwrap();
    fs::rename(t.a.join("renamed.txt"), t.a.join("editor-backup")).unwrap();
    fs::write(t.a.join("renamed.txt"), b"new identity after editor save").unwrap();
    fs::remove_file(t.a.join("editor-backup")).unwrap();
    fs::write(t.b.join("old.txt"), b"opposing edit").unwrap();
    let old = e.preview(&t.id).unwrap();
    assert_eq!(
        old.operations
            .iter()
            .find(|o| o.path == "renamed.txt")
            .unwrap()
            .action,
        Action::CopyToB
    );
    let batch = watch::EventBatch {
        renames: vec![watch::RenameHint {
            side: "A".into(),
            from: "old.txt".into(),
            to: "renamed.txt".into(),
        }],
        ..Default::default()
    };
    e.queue_watch_batch(&t.id, &batch).unwrap();
    assert!(
        e.apply(&old.id, false).is_err(),
        "Late lineage evidence must reject the earlier plan before any copy"
    );
    assert!(!t.b.join("renamed.txt").exists());
    assert_eq!(fs::read(t.b.join("old.txt")).unwrap(), b"opposing edit");
    let p = e.preview(&t.id).unwrap();
    conflict(&p, "old.txt");
    conflict(&p, "renamed.txt");
}

#[test]
fn unrelated_history_is_not_loaded_for_a_single_file_edit() {
    let (d, mut e, t) = fixture();
    let mut db = rusqlite::Connection::open(d.path().join("state/foldersync.sqlite3")).unwrap();
    let tx = db.transaction().unwrap();
    for i in 0..2000 {
        tx.execute(
            "INSERT INTO rename_links(task,origin,target) VALUES(?,?,?)",
            rusqlite::params![t.id, format!("history-old-{i}"), format!("history-new-{i}")],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    fs::write(t.a.join("old.txt"), b"one edit").unwrap();
    let p = e.preview_paths(&t.id, &["old.txt".into()]).unwrap();
    assert_eq!(p.metrics.scanned_entries, 2);
    assert_eq!(
        p.metrics.lineage_links, 0,
        "A narrow edit must not materialize disconnected historical links"
    );
    assert_eq!(p.operations.len(), 1);
    e.apply(&p.id, false).unwrap();
    assert_eq!(fs::read(t.b.join("old.txt")).unwrap(), b"one edit");
}

#[test]
fn restoring_the_old_name_allows_later_unrelated_copies_with_the_historical_content() {
    let (_d, mut e, t) = fixture();
    fs::rename(t.a.join("old.txt"), t.a.join("renamed.txt")).unwrap();
    fs::write(t.a.join("renamed.txt"), b"renamed edit").unwrap();
    fs::write(t.b.join("old.txt"), b"opposing edit").unwrap();
    let p = e.preview(&t.id).unwrap();
    let p = e.resolve(&p.id, "old.txt", "b").unwrap();
    let p = e.resolve(&p.id, "renamed.txt", "a").unwrap();
    e.apply(&p.id, true).unwrap();
    fs::write(t.a.join("unrelated.txt"), b"common").unwrap();
    let p = e.preview_paths(&t.id, &["unrelated.txt".into()]).unwrap();
    assert!(p.operations.iter().all(|o| o.action != Action::Conflict));
    e.apply(&p.id, false).unwrap();
    assert_eq!(fs::read(t.b.join("unrelated.txt")).unwrap(), b"common");
    assert_eq!(fs::read(t.a.join("old.txt")).unwrap(), b"opposing edit");
    assert_eq!(fs::read(t.b.join("renamed.txt")).unwrap(), b"renamed edit");
}

#[test]
fn successive_unique_automatic_renames_advance_the_common_identity_checkpoint() {
    let (_d, mut e, mut t) = fixture();
    t.deletion_policy = DeletionPolicy { enabled: true };
    e.save_task(&t).unwrap();
    for (from, to) in [("old.txt", "middle.txt"), ("middle.txt", "final.txt")] {
        fs::rename(t.a.join(from), t.a.join(to)).unwrap();
        e.queue_paths(&t.id, &[from.into(), to.into()], false)
            .unwrap();
        let p = e.process_queue(&t.id).unwrap().unwrap();
        assert_eq!(p.status, "Applied", "{from} -> {to}");
        assert!(!t.a.join(from).exists() && !t.b.join(from).exists());
        assert_eq!(fs::read(t.b.join(to)).unwrap(), b"common");
    }
    assert_eq!(e.pending_conflicts(&t.id).unwrap(), 0);
    assert!(
        e.versions(&t.id)
            .unwrap()
            .iter()
            .any(|v| v.path == "old.txt" && fs::read(&v.stored).unwrap() == b"common")
    );
}
