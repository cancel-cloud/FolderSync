use foldersync_engine::{watch::WatchService, *};
use std::{
    fs,
    time::{Duration, Instant},
};

fn fixture(mode: Mode) -> (tempfile::TempDir, Engine, Task) {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-auto-delete-")
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
fn seed(e: &mut Engine, t: &Task) {
    for i in 0..10 {
        fs::write(t.a.join(format!("f{i}.txt")), format!("baseline {i}")).unwrap();
    }
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
}
fn enable(e: &mut Engine, t: &mut Task) {
    t.deletion_policy = DeletionPolicy { enabled: true };
    e.save_task(t).unwrap();
}
fn queue(e: &Engine, t: &Task, paths: &[&str]) {
    e.queue_paths(
        &t.id,
        &paths.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
        false,
    )
    .unwrap();
}
fn native_path(w: &WatchService, path: &str) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if w.poll().paths.iter().any(|p| p == path) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("No native event for {path}");
}
#[test]
fn automatic_baselines_do_not_substitute_for_explicit_manual_approval() {
    let (_d, mut e, mut t) = fixture(Mode::TwoWay);
    t.deletion_policy.enabled = true;
    e.save_task(&t).unwrap();
    fs::write(t.a.join("auto.txt"), "automatic baseline").unwrap();
    e.queue_paths(&t.id, &[], true).unwrap();
    assert_eq!(e.process_queue(&t.id).unwrap().unwrap().status, "Applied");
    fs::remove_file(t.a.join("auto.txt")).unwrap();
    queue(&e, &t, &["auto.txt"]);
    let blocked = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(blocked.status, "Review required");
    assert_eq!(
        blocked.deletion_review.unwrap().reason,
        DeletionReason::BaselineApprovalRequired
    );
    assert!(
        t.b.join("auto.txt").exists(),
        "An unattended scan is not manual approval"
    );
    drop(e);
    let mut e = Engine::open(&_d.path().join("state")).unwrap();
    assert!(e.tasks().unwrap()[0].deletion_policy.enabled);
    queue(&e, &t, &["auto.txt"]);
    assert_eq!(
        e.process_queue(&t.id).unwrap().unwrap().status,
        "Review required"
    );
    fs::write(t.a.join("auto.txt"), "automatic baseline").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    fs::remove_file(t.a.join("auto.txt")).unwrap();
    queue(&e, &t, &["auto.txt"]);
    assert_eq!(e.process_queue(&t.id).unwrap().unwrap().status, "Applied");
    assert!(!t.b.join("auto.txt").exists());
}
#[test]
fn native_both_side_deletions_are_protected_and_conflicts_still_preserve_edits() {
    let (_d, mut e, mut t) = fixture(Mode::TwoWay);
    seed(&mut e, &t);
    enable(&mut e, &mut t);
    let watch = WatchService::new(&t).unwrap();
    fs::remove_file(t.a.join("f0.txt")).unwrap();
    native_path(&watch, "f0.txt");
    queue(&e, &t, &["f0.txt"]);
    let p = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(p.status, "Applied");
    assert_eq!(p.metrics.reconciliations, 0);
    assert_eq!(
        p.deletion_review.as_ref().unwrap().reason,
        DeletionReason::Approved
    );
    assert!(!t.b.join("f0.txt").exists());
    assert!(
        e.versions(&t.id)
            .unwrap()
            .iter()
            .any(|v| v.path == "f0.txt" && fs::read(&v.stored).unwrap() == b"baseline 0")
    );
    fs::remove_file(t.b.join("f1.txt")).unwrap();
    native_path(&watch, "f1.txt");
    queue(&e, &t, &["f1.txt"]);
    e.process_queue(&t.id).unwrap();
    assert!(!t.a.join("f1.txt").exists());
    fs::write(t.a.join("f2.txt"), "edited after baseline").unwrap();
    fs::remove_file(t.b.join("f2.txt")).unwrap();
    fs::write(t.a.join("independent.txt"), "independent").unwrap();
    queue(&e, &t, &["f2.txt", "independent.txt"]);
    let p = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(p.status, "Attention");
    assert_eq!(
        fs::read(t.a.join("f2.txt")).unwrap(),
        b"edited after baseline"
    );
    assert!(!t.b.join("f2.txt").exists());
    assert_eq!(
        fs::read(t.b.join("independent.txt")).unwrap(),
        b"independent"
    );
    assert_eq!(e.pending_conflicts(&t.id).unwrap(), 1);
}
#[test]
fn proven_bulk_deletion_has_no_entry_or_percentage_cap() {
    let (_d, mut e, mut t) = fixture(Mode::TwoWay);
    for i in 0..40 {
        fs::write(t.a.join(format!("bulk-{i}")), format!("content {i}")).unwrap();
    }
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    enable(&mut e, &mut t);
    for i in 0..40 {
        fs::remove_file(t.a.join(format!("bulk-{i}"))).unwrap();
    }
    e.queue_paths(&t.id, &[], true).unwrap();
    let p = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(p.status, "Applied");
    assert_eq!(
        p.deletion_review.as_ref().unwrap().reason,
        DeletionReason::Approved
    );
    assert_eq!(p.deletion_review.as_ref().unwrap().entries, 40);
    assert_eq!(fs::read_dir(&t.b).unwrap().count(), 0);
    assert_eq!(e.versions(&t.id).unwrap().len(), 40);
}

#[test]
fn all_proven_files_may_be_removed_when_the_root_is_still_reachable() {
    let (_d, mut e, mut t) = fixture(Mode::TwoWay);
    seed(&mut e, &t);
    enable(&mut e, &mut t);
    for i in 0..10 {
        fs::remove_file(t.a.join(format!("f{i}.txt"))).unwrap();
    }
    e.queue_paths(&t.id, &[], true).unwrap();
    let p = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(p.status, "Applied");
    assert_eq!(p.deletion_review.unwrap().reason, DeletionReason::Approved);
    assert!(t.a.is_dir() && t.b.is_dir());
    assert_eq!(fs::read_dir(&t.b).unwrap().count(), 0);
}

#[test]
fn mirror_extras_without_a_common_baseline_remain_manual() {
    let (_d, mut e, mut t) = fixture(Mode::Mirror);
    seed(&mut e, &t);
    enable(&mut e, &mut t);
    fs::write(t.b.join("unproven.txt"), "destination only").unwrap();
    fs::remove_file(t.a.join("f0.txt")).unwrap();
    queue(&e, &t, &["unproven.txt", "f0.txt"]);
    let p = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(p.status, "Review required");
    assert_eq!(
        p.deletion_review.as_ref().unwrap().reason,
        DeletionReason::UnprovenDeletion
    );
    assert!(t.b.join("f0.txt").exists());
    assert_eq!(
        fs::read(t.b.join("unproven.txt")).unwrap(),
        b"destination only"
    );
}
#[test]
fn policy_and_manual_approval_persist_but_imports_cannot_enable_deletion() {
    let (d, mut e, mut t) = fixture(Mode::TwoWay);
    seed(&mut e, &t);
    enable(&mut e, &mut t);
    let export = e.export().unwrap();
    drop(e);
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    assert!(e.tasks().unwrap()[0].deletion_policy.enabled);
    fs::remove_file(t.a.join("f0.txt")).unwrap();
    e.queue_paths(&t.id, &[], true).unwrap();
    assert_eq!(e.process_queue(&t.id).unwrap().unwrap().status, "Applied");
    e.import(&export).unwrap();
    assert!(
        !e.tasks()
            .unwrap()
            .iter()
            .find(|x| x.id != t.id)
            .unwrap()
            .deletion_policy
            .enabled
    );
}
#[test]
fn scope_changes_require_new_manual_review_and_move_cannot_opt_in() {
    let (_d, mut e, mut t) = fixture(Mode::TwoWay);
    seed(&mut e, &t);
    enable(&mut e, &mut t);
    t.scope.exclude.push("*.tmp".into());
    e.save_task(&t).unwrap();
    fs::remove_file(t.a.join("f0.txt")).unwrap();
    queue(&e, &t, &["f0.txt"]);
    let blocked = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(
        blocked.deletion_review.unwrap().reason,
        DeletionReason::BaselineApprovalRequired
    );
    assert!(t.b.join("f0.txt").exists());
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    enable(&mut e, &mut t);
    let (_move_d, mut move_e, mut move_t) = fixture(Mode::Move);
    seed(&mut move_e, &move_t);
    move_t.deletion_policy.enabled = true;
    assert!(move_e.save_task(&move_t).is_err());
}

#[test]
fn nested_directory_removal_has_no_descendant_count_cap() {
    let (_d, mut e, mut t) = fixture(Mode::TwoWay);
    fs::create_dir(t.a.join("nested")).unwrap();
    for i in 0..30 {
        fs::write(t.a.join(format!("nested/f{i}")), "nested bytes").unwrap();
    }
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    enable(&mut e, &mut t);
    fs::remove_dir_all(t.a.join("nested")).unwrap();
    queue(&e, &t, &["nested"]);
    let p = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(p.status, "Applied");
    let decision = p.deletion_review.as_ref().unwrap();
    assert_eq!(decision.entries, 31);
    assert_eq!(decision.reason, DeletionReason::Approved);
    assert!(!t.b.join("nested").exists());
    assert_eq!(
        e.plan_header(&p.id)
            .unwrap()
            .deletion_review
            .unwrap()
            .reason,
        DeletionReason::Approved
    );
}

#[test]
fn enabled_policy_never_turns_an_unavailable_root_into_deletion_evidence() {
    let (d, mut e, mut t) = fixture(Mode::TwoWay);
    seed(&mut e, &t);
    enable(&mut e, &mut t);
    let parked = d.path().join("A-offline");
    fs::rename(&t.a, &parked).unwrap();
    e.queue_paths(&t.id, &[], true).unwrap();
    assert!(e.process_queue(&t.id).is_err());
    assert_eq!(fs::read_dir(&t.b).unwrap().count(), 10);
    assert_eq!(e.queued(&t.id).unwrap(), vec!["*"]);
    fs::rename(&parked, &t.a).unwrap();
    assert_eq!(e.process_queue(&t.id).unwrap().unwrap().status, "Applied");
    assert_eq!(fs::read_dir(&t.b).unwrap().count(), 10);
}

#[test]
fn reappearing_deleted_side_during_backup_preserves_both_files() {
    let (_d, mut e, mut t) = fixture(Mode::TwoWay);
    let bytes = vec![37; 2 * 1024 * 1024];
    fs::write(t.a.join("large.bin"), &bytes).unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    enable(&mut e, &mut t);
    t.bandwidth_kib = 512;
    e.save_task(&t).unwrap();
    fs::remove_file(t.a.join("large.bin")).unwrap();
    queue(&e, &t, &["large.bin"]);
    let progress = e.progress.clone();
    let cancel = e.cancel.clone();
    let reappearing = t.a.join("large.bin");
    let observer = std::thread::spawn(move || {
        let start = Instant::now();
        loop {
            if progress.snapshot().is_some_and(|s| {
                s.phase == ProgressPhase::Protecting
                    && s.file_bytes > 0
                    && s.file_bytes < s.file_total
            }) {
                fs::write(reappearing, "new concurrent edit").unwrap();
                return;
            }
            if start.elapsed() > Duration::from_secs(5) {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                panic!("No protected-copy progress observed");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let result = e.process_queue(&t.id);
    observer.join().unwrap();
    assert!(
        result.is_err(),
        "A reappearing deleted side must stop deletion"
    );
    assert_eq!(
        fs::read(t.a.join("large.bin")).unwrap(),
        b"new concurrent edit"
    );
    assert_eq!(fs::read(t.b.join("large.bin")).unwrap(), bytes);
}

#[test]
fn legacy_limit_values_are_ignored_and_not_exported_again() {
    let (_d, mut e, t) = fixture(Mode::TwoWay);
    seed(&mut e, &t);
    let mut json = serde_json::to_value(&t).unwrap();
    json["deletion_policy"] = serde_json::json!({"enabled":true,"max_entries":0,"max_percent":0});
    let t: Task = serde_json::from_value(json).unwrap();
    e.save_task(&t).unwrap();
    let saved = serde_json::to_value(&e.tasks().unwrap()[0]).unwrap();
    assert_eq!(
        saved["deletion_policy"],
        serde_json::json!({"enabled":true})
    );
    fs::remove_file(t.a.join("f0.txt")).unwrap();
    queue(&e, &t, &["f0.txt"]);
    assert_eq!(e.process_queue(&t.id).unwrap().unwrap().status, "Applied");
}

#[test]
fn bulk_external_moves_propagate_new_paths_and_remove_old_paths_without_caps() {
    let (_d, mut e, mut t) = fixture(Mode::TwoWay);
    for i in 0..30 {
        fs::write(t.a.join(format!("old-{i}")), format!("unique {i}")).unwrap();
    }
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    enable(&mut e, &mut t);
    let mut paths = vec![];
    for i in 0..30 {
        let old = format!("old-{i}");
        let new = format!("new-{i}");
        fs::rename(t.a.join(&old), t.a.join(&new)).unwrap();
        paths.extend([old, new]);
    }
    e.queue_paths(&t.id, &paths, false).unwrap();
    let p = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(p.status, "Applied");
    for i in 0..30 {
        assert!(!t.b.join(format!("old-{i}")).exists());
        assert_eq!(
            fs::read_to_string(t.b.join(format!("new-{i}"))).unwrap(),
            format!("unique {i}")
        );
    }
}
