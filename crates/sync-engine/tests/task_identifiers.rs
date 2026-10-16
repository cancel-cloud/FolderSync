use foldersync_engine::*;
use std::fs;
fn with_identifier(task: &Task, value: &str) -> Task {
    let mut json = serde_json::to_value(task).unwrap();
    json["identifier"] = value.into();
    serde_json::from_value(json).unwrap()
}

#[test]
fn identifiers_roundtrip_and_colliding_imports_get_new_automatic_identifiers() {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-ID-import-")
        .tempdir()
        .unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let mut engine = Engine::open(&d.path().join("state")).unwrap();
    let task = with_identifier(
        &Task {
            a,
            b,
            ..Task::default()
        },
        "CAD-01",
    );
    engine.save_task(&task).unwrap();
    let export = engine.export().unwrap();
    let mut imported = Engine::open(&d.path().join("import-state")).unwrap();
    imported.import(&export).unwrap();
    let first = imported.tasks().unwrap().remove(0);
    assert_eq!(
        serde_json::to_value(&first).unwrap()["identifier"],
        "CAD-01"
    );
    assert_ne!(first.id, task.id);
    imported.import(&export).unwrap();
    let copies = imported.tasks().unwrap();
    assert_eq!(copies.len(), 2);
    assert_eq!(
        copies
            .iter()
            .filter(|t| serde_json::to_value(t).unwrap()["identifier"] == "CAD-01")
            .count(),
        1
    );
    assert_eq!(
        copies
            .iter()
            .filter(|t| serde_json::to_value(t).unwrap()["identifier"] == "")
            .count(),
        1
    );
    let mut legacy = serde_json::to_value(&task).unwrap();
    legacy.as_object_mut().unwrap().remove("identifier");
    let legacy: Task = serde_json::from_value(legacy).unwrap();
    assert_eq!(serde_json::to_value(legacy).unwrap()["identifier"], "");
}
#[test]
fn editable_identifiers_preserve_baselines_and_history_and_reject_duplicates() {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-identifiers-")
        .tempdir()
        .unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let mut engine = Engine::open(&d.path().join("state")).unwrap();
    let task = with_identifier(
        &Task {
            a,
            b,
            ..Task::default()
        },
        "CAD-東京-01",
    );
    engine.save_task(&task).unwrap();
    fs::write(task.a.join("proof"), "baseline").unwrap();
    let first = engine.preview(&task.id).unwrap();
    engine.apply(&first.id, false).unwrap();
    let preview = engine.preview(&task.id).unwrap();
    let edited = with_identifier(&task, "CAD-東京-02");
    engine.save_task(&edited).unwrap();
    assert_eq!(
        serde_json::to_value(engine.tasks().unwrap().remove(0)).unwrap()["identifier"],
        "CAD-東京-02"
    );
    assert!(engine.apply(&preview.id, false).is_err());
    assert_eq!(
        serde_json::to_value(
            engine
                .history(&task.id)
                .unwrap()
                .into_iter()
                .find(|p| p.id == first.id)
                .unwrap()
        )
        .unwrap()["task"]["identifier"],
        "CAD-東京-01"
    );
    let next = engine.preview(&task.id).unwrap();
    assert!(
        next.operations
            .iter()
            .all(|o| o.action == Action::Unchanged)
    );
    let mut duplicate = with_identifier(&task, "cad-東京-02");
    duplicate.id = uuid::Uuid::new_v4().to_string();
    assert!(engine.save_task(&duplicate).is_err());
    for invalid in [" bad ", "line\nbreak", &"x".repeat(65), &"🚀".repeat(65)] {
        assert!(
            engine
                .save_task(&with_identifier(&edited, invalid))
                .is_err()
        );
    }
    engine
        .save_task(&with_identifier(&edited, &"🚀".repeat(64)))
        .unwrap();
    engine.save_task(&edited).unwrap();
    drop(engine);
    let engine = Engine::open(&d.path().join("state")).unwrap();
    assert_eq!(
        serde_json::to_value(engine.tasks().unwrap().remove(0)).unwrap()["identifier"],
        "CAD-東京-02"
    );
    assert_eq!(fs::read(task.b.join("proof")).unwrap(), b"baseline");
}
