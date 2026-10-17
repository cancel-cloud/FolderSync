use foldersync_engine::*;
use std::fs;

fn fixture() -> (tempfile::TempDir, Engine, Task) {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-task-policies-")
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
    (d, e, t)
}
fn policy(t: &Task, comparison: &str, conflicts: &str) -> Task {
    let mut json = serde_json::to_value(t).unwrap();
    json["comparison"] = comparison.into();
    json["conflict_policy"] = conflicts.into();
    serde_json::from_value(json).unwrap()
}

#[test]
fn full_content_policy_persists_and_detects_unnotified_timestamp_preserving_edit() {
    let (d, mut e, mut t) = fixture();
    for i in 0..20 {
        fs::write(t.a.join(format!("f{i}")), format!("base{i:02}")).unwrap();
    }
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    let stale = e.preview(&t.id).unwrap();
    t = policy(&t, "FullContent", "ContinueIndependent");
    e.save_task(&t).unwrap();
    assert!(e.apply(&stale.id, false).is_err());
    drop(e);
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    let file = t.a.join("f19");
    let before = filetime::FileTime::from_last_modification_time(&fs::metadata(&file).unwrap());
    fs::write(&file, b"edit19").unwrap();
    filetime::set_file_mtime(&file, before).unwrap();
    // Only f0 is reported. Full policy must still verify the unreported sibling.
    let p = e.preview_paths(&t.id, &["f0".into()]).unwrap();
    assert_eq!(p.metrics.reconciliations, 1);
    assert_eq!(p.metrics.scanned_entries, 40);
    assert!(
        p.operations
            .iter()
            .any(|o| o.path == "f19" && o.action == Action::CopyToB)
    );
    e.apply(&p.id, false).unwrap();
    assert_eq!(fs::read(t.b.join("f19")).unwrap(), b"edit19");
}

#[test]
fn require_choices_holds_entire_run_without_baseline_or_file_changes_until_reviewed() {
    let (_d, mut e, mut t) = fixture();
    fs::write(t.a.join("conflict"), b"base").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    t = policy(&t, "Incremental", "RequireChoices");
    e.save_task(&t).unwrap();
    fs::write(t.a.join("conflict"), b"left").unwrap();
    fs::write(t.b.join("conflict"), b"right").unwrap();
    fs::write(t.a.join("independent"), b"new").unwrap();
    e.queue_paths(&t.id, &["conflict".into(), "independent".into()], false)
        .unwrap();
    let p = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(p.status, "Review required");
    assert!(!t.b.join("independent").exists());
    assert!(e.apply(&p.id, true).is_err());
    assert_eq!(fs::read(t.a.join("conflict")).unwrap(), b"left");
    assert_eq!(fs::read(t.b.join("conflict")).unwrap(), b"right");
    assert!(e.versions(&t.id).unwrap().is_empty());
    let p = e.resolve(&p.id, "conflict", "ignore").unwrap();
    assert!(!t.b.join("independent").exists());
    assert!(e.apply(&p.id, false).is_err());
    let p = e.apply(&p.id, true).unwrap();
    assert_eq!(p.status, "Attention");
    assert_eq!(fs::read(t.b.join("independent")).unwrap(), b"new");
    let p = e.preview(&t.id).unwrap();
    assert!(
        p.operations
            .iter()
            .any(|o| o.path == "conflict" && o.action == Action::Conflict)
    );
}

#[test]
fn strict_policy_survives_export_import_and_restart_without_enabling_automation() {
    let (d, mut e, t) = fixture();
    let t = policy(&t, "FullContent", "RequireChoices");
    e.save_task(&t).unwrap();
    let export = e.export().unwrap();
    drop(e);
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    e.import(&export).unwrap();
    fs::write(t.a.join("conflict"), b"left").unwrap();
    fs::write(t.b.join("conflict"), b"right").unwrap();
    fs::write(t.a.join("independent"), b"new").unwrap();
    for task in e.tasks().unwrap() {
        let p = e.preview_paths(&task.id, &["independent".into()]).unwrap();
        assert_eq!(p.metrics.reconciliations, 1);
        assert!(!p.task.monitor && !p.task.login && p.task.schedule.kind.is_empty());
        assert!(e.apply(&p.id, true).is_err());
        assert!(!t.b.join("independent").exists());
    }
}

#[test]
fn conflict_hold_overrides_approved_automatic_deletion_until_explicit_decision() {
    let (_d, mut e, mut t) = fixture();
    for name in ["conflict", "remove"] {
        fs::write(t.a.join(name), b"base").unwrap();
    }
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    t = policy(&t, "Incremental", "RequireChoices");
    t.deletion_policy = DeletionPolicy { enabled: true };
    e.save_task(&t).unwrap();
    fs::write(t.a.join("conflict"), b"left").unwrap();
    fs::write(t.b.join("conflict"), b"right").unwrap();
    fs::remove_file(t.a.join("remove")).unwrap();
    e.queue_paths(&t.id, &["conflict".into(), "remove".into()], false)
        .unwrap();
    let p = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(
        p.deletion_review.as_ref().unwrap().reason,
        DeletionReason::Approved
    );
    assert_eq!(p.status, "Review required");
    assert!(e.apply(&p.id, true).is_err());
    assert_eq!(fs::read(t.b.join("remove")).unwrap(), b"base");
    assert!(e.versions(&t.id).unwrap().is_empty());
    let p = e.resolve(&p.id, "conflict", "ignore").unwrap();
    e.apply(&p.id, true).unwrap();
    assert!(!t.b.join("remove").exists());
    assert!(
        e.versions(&t.id)
            .unwrap()
            .iter()
            .any(|v| v.path == "remove" && fs::read(&v.stored).unwrap() == b"base")
    );
}

#[test]
fn strict_hold_carries_known_conflicts_across_batches_and_rejects_older_partial_plans() {
    let (_d, mut e, mut t) = fixture();
    t = policy(&t, "Incremental", "RequireChoices");
    e.save_task(&t).unwrap();
    fs::write(t.a.join("conflict"), b"left").unwrap();
    fs::write(t.b.join("conflict"), b"right").unwrap();
    fs::write(t.a.join("independent"), b"new").unwrap();
    let older = e.preview_paths(&t.id, &["independent".into()]).unwrap();
    e.queue_paths(&t.id, &["conflict".into()], false).unwrap();
    assert_eq!(
        e.process_queue(&t.id).unwrap().unwrap().status,
        "Review required"
    );
    assert!(e.apply(&older.id, true).is_err());
    assert!(!t.b.join("independent").exists());
    e.queue_paths(&t.id, &["independent".into()], false)
        .unwrap();
    let p = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(p.status, "Review required");
    assert!(!t.b.join("independent").exists());
    assert!(
        p.operations
            .iter()
            .any(|o| o.path == "conflict" && o.action == Action::Conflict)
    );
    let p = e.resolve(&p.id, "conflict", "ignore").unwrap();
    e.apply(&p.id, true).unwrap();
    assert_eq!(fs::read(t.b.join("independent")).unwrap(), b"new");
    // The first-run conflict had no common baseline. External removal from
    // both roots must still be observed and clear the durable attention row.
    fs::remove_file(t.a.join("conflict")).unwrap();
    fs::remove_file(t.b.join("conflict")).unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    assert_eq!(e.pending_conflicts(&t.id).unwrap(), 0);
}

#[test]
fn explicitly_excluded_conflict_does_not_leave_an_unreviewable_strict_hold() {
    let (_d, mut e, mut t) = fixture();
    t = policy(&t, "Incremental", "RequireChoices");
    e.save_task(&t).unwrap();
    fs::write(t.a.join("conflict"), b"left").unwrap();
    fs::write(t.b.join("conflict"), b"right").unwrap();
    e.preview(&t.id).unwrap();
    assert_eq!(e.pending_conflicts(&t.id).unwrap(), 1);
    t.scope.exclude.push("conflict".into());
    e.save_task(&t).unwrap();
    fs::write(t.a.join("independent"), b"new").unwrap();
    let p = e.preview_paths(&t.id, &["independent".into()]).unwrap();
    e.apply(&p.id, false).unwrap();
    assert_eq!(fs::read(t.b.join("independent")).unwrap(), b"new");
    assert_eq!(e.pending_conflicts(&t.id).unwrap(), 0);
    assert_eq!(fs::read(t.a.join("conflict")).unwrap(), b"left");
    assert_eq!(fs::read(t.b.join("conflict")).unwrap(), b"right");
    // Returning the path to scope rediscoveres the still-different originals.
    t.scope.exclude.clear();
    e.save_task(&t).unwrap();
    let p = e.preview(&t.id).unwrap();
    assert!(
        p.operations
            .iter()
            .any(|o| o.path == "conflict" && o.action == Action::Conflict)
    );
}

#[cfg(windows)]
#[test]
fn attribute_exclusion_releases_strict_hold_without_changing_either_original() {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::SetFileAttributesW;
    for attribute in [2, 4] {
        let (_d, mut e, mut t) = fixture();
        t = policy(&t, "Incremental", "RequireChoices");
        e.save_task(&t).unwrap();
        fs::write(t.a.join("conflict"), b"left").unwrap();
        fs::write(t.b.join("conflict"), b"right").unwrap();
        e.preview(&t.id).unwrap();
        assert_eq!(e.pending_conflicts(&t.id).unwrap(), 1);
        let path: Vec<u16> =
            t.a.join("conflict")
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect();
        assert_ne!(unsafe { SetFileAttributesW(path.as_ptr(), attribute) }, 0);
        t.scope.skip_hidden = attribute == 2;
        t.scope.skip_system = attribute == 4;
        e.save_task(&t).unwrap();
        fs::write(t.a.join("independent"), b"new").unwrap();
        let p = e.preview_paths(&t.id, &["independent".into()]).unwrap();
        e.apply(&p.id, false).unwrap();
        assert_eq!(e.pending_conflicts(&t.id).unwrap(), 0);
        assert_eq!(fs::read(t.b.join("independent")).unwrap(), b"new");
        assert_eq!(fs::read(t.a.join("conflict")).unwrap(), b"left");
        assert_eq!(fs::read(t.b.join("conflict")).unwrap(), b"right");
        assert_ne!(unsafe { SetFileAttributesW(path.as_ptr(), 128) }, 0);
    }
}
