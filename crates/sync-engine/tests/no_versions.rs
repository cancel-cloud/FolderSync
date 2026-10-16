use foldersync_engine::*;
use std::fs;

fn fixture() -> (tempfile::TempDir, Engine, Task) {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    let mut json = serde_json::to_value(Task {
        a,
        b,
        ..Task::default()
    })
    .unwrap();
    json["history_enabled"] = false.into();
    let t: Task = serde_json::from_value(json).unwrap();
    e.save_task(&t).unwrap();
    (d, e, t)
}

fn apply(e: &mut Engine, t: &Task) {
    let p = e.preview(&t.id).unwrap();
    assert!(!p.operations.iter().any(|o| o.action == Action::Conflict));
    e.apply(&p.id, true).unwrap();
}

#[test]
fn disabled_history_syncs_updates_and_deletions_without_retained_versions() {
    let (_d, mut e, t) = fixture();
    fs::write(t.a.join("file"), "initial").unwrap();
    apply(&mut e, &t);
    fs::write(t.a.join("file"), "updated").unwrap();
    apply(&mut e, &t);
    assert_eq!(fs::read(t.b.join("file")).unwrap(), b"updated");
    assert!(e.versions(&t.id).unwrap().is_empty());
    fs::remove_file(t.a.join("file")).unwrap();
    apply(&mut e, &t);
    assert!(!t.b.join("file").exists());
    assert!(e.versions(&t.id).unwrap().is_empty());
    assert_eq!(
        fs::read_dir(e.storage().join("versions")).unwrap().count(),
        0
    );
}

#[test]
fn legacy_tasks_keep_history_enabled_and_disabled_setting_exports_and_imports() {
    let (_d, e, t) = fixture();
    let mut legacy = serde_json::to_value(Task::default()).unwrap();
    legacy.as_object_mut().unwrap().remove("history_enabled");
    assert!(
        serde_json::from_value::<Task>(legacy)
            .unwrap()
            .history_enabled
    );
    assert!(!e.tasks().unwrap()[0].history_enabled);
    let export = e.export().unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut imported = Engine::open(other.path()).unwrap();
    imported.import(&export).unwrap();
    assert!(!imported.tasks().unwrap()[0].history_enabled);
    assert_eq!(imported.tasks().unwrap()[0].a, t.a);
}

#[test]
fn disabling_history_preserves_existing_versions_and_survives_restart() {
    let (d, mut e, mut t) = fixture();
    let mut json = serde_json::to_value(&t).unwrap();
    json["history_enabled"] = true.into();
    t = serde_json::from_value(json).unwrap();
    e.save_task(&t).unwrap();
    fs::write(t.a.join("file"), "initial").unwrap();
    apply(&mut e, &t);
    fs::write(t.a.join("file"), "version two").unwrap();
    apply(&mut e, &t);
    let before = e.versions(&t.id).unwrap();
    assert!(!before.is_empty());
    let mut json = serde_json::to_value(&t).unwrap();
    json["history_enabled"] = false.into();
    t = serde_json::from_value(json).unwrap();
    e.save_task(&t).unwrap();
    drop(e);
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    fs::write(t.a.join("file"), "version three").unwrap();
    apply(&mut e, &t);
    let after = e.versions(&t.id).unwrap();
    assert_eq!(before.len(), after.len());
    assert!(before.iter().all(|v| after.iter().any(|a| a.id == v.id)));
    for version in &after {
        assert_eq!(fs::read(&version.stored).unwrap(), b"initial");
    }
    let restore = e.restore_preview(&after[0].id).unwrap();
    e.restore_apply(&restore.id).unwrap();
    assert_eq!(fs::read(t.b.join("file")).unwrap(), b"initial");
    assert_eq!(fs::read(t.a.join("file")).unwrap(), b"version three");
    assert_eq!(e.versions(&t.id).unwrap().len(), before.len());
}
