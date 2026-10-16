use foldersync_engine::*;
use std::fs;
fn fixture() -> (tempfile::TempDir, Engine, Task) {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-review-")
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
    (d, e, t)
}
#[test]
fn case_spelling_across_roots_is_not_stale() {
    let (_d, mut e, t) = fixture();
    fs::write(t.a.join("Foo.txt"), "base").unwrap();
    fs::write(t.b.join("foo.txt"), "base").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    fs::write(t.a.join("Foo.txt"), "updated").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    assert_eq!(fs::read(t.b.join("foo.txt")).unwrap(), b"updated");
}
#[test]
fn explicit_restore_preserves_current_bytes_even_after_scope_exclusion() {
    let (_d, mut e, mut t) = fixture();
    fs::write(t.a.join("f"), "base").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    fs::write(t.a.join("f"), "update").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    let v = e.versions(&t.id).unwrap().remove(0);
    t.scope.exclude = vec!["f".into()];
    e.save_task(&t).unwrap();
    fs::write(t.b.join("f"), "precious excluded edit").unwrap();
    let p = e.restore_preview(&v.id).unwrap();
    assert!(p.current.is_some());
    e.restore_apply(&p.id).unwrap();
    assert!(
        e.versions(&t.id)
            .unwrap()
            .iter()
            .any(|v| fs::read(&v.stored).unwrap() == b"precious excluded edit")
    );
}
#[cfg(windows)]
#[test]
fn filtered_hidden_files_are_not_deletion_or_absence_signals() {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::SetFileAttributesW;
    let (_d, mut e, mut t) = fixture();
    t.scope.skip_hidden = true;
    e.save_task(&t).unwrap();
    fs::write(t.a.join("f"), "base").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    let path: Vec<_> =
        t.a.join("f")
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
    assert_ne!(unsafe { SetFileAttributesW(path.as_ptr(), 2) }, 0);
    let p = e.preview(&t.id).unwrap();
    assert!(
        p.operations.is_empty(),
        "excluded A must not delete visible B"
    );
    e.apply(&p.id, true).unwrap();
    assert_eq!(fs::read(t.b.join("f")).unwrap(), b"base");
}
#[test]
fn basic_logs_omit_paths_and_detailed_logs_are_explicit() {
    let (_d, mut e, t) = fixture();
    fs::write(t.a.join("private-file"), "base").unwrap();
    e.set_setting("log_level", "basic").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    let log = fs::read_to_string(e.storage().join("operations.ndjson")).unwrap();
    assert!(!log.contains("private-file"));
    assert!(!log.contains(&t.a.to_string_lossy().to_string()));
    e.set_setting("log_level", "detailed").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    assert!(
        fs::read_to_string(e.storage().join("operations.ndjson"))
            .unwrap()
            .contains("private-file")
    );
}
#[cfg(windows)]
#[test]
fn read_only_destination_fails_without_losing_either_side() {
    let (_d, mut e, t) = fixture();
    fs::write(t.a.join("f"), "base").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    fs::write(t.a.join("f"), "new").unwrap();
    let p = e.preview(&t.id).unwrap();
    let target = t.b.join("f");
    let original_permissions = fs::metadata(&target).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    fs::set_permissions(&target, permissions).unwrap();
    let result = e.apply(&p.id, true);
    fs::set_permissions(&target, original_permissions).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read(&target).unwrap(), b"base");
    assert_eq!(fs::read(t.a.join("f")).unwrap(), b"new");
}
#[test]
fn uncertain_duplicate_events_survive_offline_root_then_reconnect_safely() {
    let (d, mut e, t) = fixture();
    fs::write(t.a.join("f"), "base").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    e.queue_paths(&t.id, &["f".into(), "f".into()], true)
        .unwrap();
    assert_eq!(e.queued(&t.id).unwrap().len(), 2);
    let away = d.path().join("A-unavailable");
    fs::rename(&t.a, &away).unwrap();
    assert!(e.process_queue(&t.id).is_err());
    assert_eq!(fs::read(t.b.join("f")).unwrap(), b"base");
    assert!(!e.queued(&t.id).unwrap().is_empty());
    fs::rename(&away, &t.a).unwrap();
    fs::write(t.a.join("f"), "reconnected edit").unwrap();
    let p = e.process_queue(&t.id).unwrap().unwrap();
    assert_eq!(p.metrics.reconciliations, 1);
    assert_eq!(fs::read(t.b.join("f")).unwrap(), b"reconnected edit");
    assert!(e.queued(&t.id).unwrap().is_empty());
}
