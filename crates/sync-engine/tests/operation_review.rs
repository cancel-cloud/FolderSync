use foldersync_engine::*;
use std::fs;
fn fixture() -> (tempfile::TempDir, Engine, Task) {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-operation-review-")
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
fn review_exposes_creation_update_and_directory_types_through_persistence_and_paging() {
    let (_d, mut e, t) = fixture();
    fs::write(t.a.join("new"), "first").unwrap();
    fs::create_dir(t.a.join("empty")).unwrap();
    let p = e.preview(&t.id).unwrap();
    let original = p.operations.iter().find(|o| o.path == "new").unwrap();
    let mut legacy = serde_json::to_value(original).unwrap();
    legacy.as_object_mut().unwrap().remove("kind");
    let legacy: Operation = serde_json::from_value(legacy).unwrap();
    assert_eq!(serde_json::to_value(&legacy).unwrap()["kind"], "Create");
    let mut forged = serde_json::to_value(original).unwrap();
    forged["kind"] = "DeleteToTrash".into();
    let forged: Operation = serde_json::from_value(forged).unwrap();
    assert_eq!(serde_json::to_value(&forged).unwrap()["kind"], "Create");
    let page = e.operation_page(&p.id, &OperationQuery::default()).unwrap();
    let json = serde_json::to_value(&page).unwrap();
    assert_eq!(
        json["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["path"] == "new")
            .unwrap()["kind"],
        "Create"
    );
    assert_eq!(
        json["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["path"] == "empty")
            .unwrap()["kind"],
        "Mkdir"
    );
    e.apply(&p.id, false).unwrap();
    fs::write(t.a.join("new"), "updated").unwrap();
    let p = e.preview(&t.id).unwrap();
    assert_eq!(
        serde_json::to_value(p.operations.iter().find(|o| o.path == "new").unwrap()).unwrap()["kind"],
        "Update"
    );
    e.apply(&p.id, false).unwrap();
    fs::remove_file(t.a.join("new")).unwrap();
    let p = e.preview(&t.id).unwrap();
    assert_eq!(
        serde_json::to_value(p.operations.iter().find(|o| o.path == "new").unwrap()).unwrap()["kind"],
        "DeleteToTrash"
    );
}
#[test]
fn ignore_skips_only_this_review_preserves_baseline_and_keeps_conflict_attention() {
    let (d, mut e, t) = fixture();
    fs::write(t.a.join("conflict"), "baseline").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    fs::write(t.a.join("conflict"), "A edit").unwrap();
    fs::write(t.b.join("conflict"), "B edit").unwrap();
    fs::write(t.a.join("independent"), "safe copy").unwrap();
    let p = e.preview(&t.id).unwrap();
    let ignored = e.resolve(&p.id, "conflict", "ignore").unwrap();
    let header = e.plan_header(&ignored.id).unwrap();
    assert_eq!((header.summary.conflicts, header.summary.ignored), (1, 1));
    let memory = PlanHeader::from(&ignored);
    assert_eq!((memory.summary.conflicts, memory.summary.ignored), (1, 1));
    let kept = e.resolve(&ignored.id, "conflict", "keep").unwrap();
    assert_eq!(e.plan_header(&kept.id).unwrap().summary.ignored, 0);
    let ignored = e.resolve(&kept.id, "conflict", "ignore").unwrap();
    assert_eq!(
        serde_json::to_value(
            ignored
                .operations
                .iter()
                .find(|o| o.path == "conflict")
                .unwrap()
        )
        .unwrap()["action"],
        "Ignore"
    );
    assert_eq!(fs::read(t.a.join("conflict")).unwrap(), b"A edit");
    assert_eq!(fs::read(t.b.join("conflict")).unwrap(), b"B edit");
    assert!(!t.b.join("independent").exists());
    let applied = e.apply(&ignored.id, true).unwrap();
    assert_eq!(applied.status, "Attention");
    assert_eq!(
        applied
            .operations
            .iter()
            .find(|o| o.path == "conflict")
            .unwrap()
            .status,
        "Ignored"
    );
    assert_eq!(fs::read(t.b.join("independent")).unwrap(), b"safe copy");
    assert_eq!(e.statuses().unwrap()[0].pending_conflicts, 1);
    let next = e.preview(&t.id).unwrap();
    assert_eq!(
        next.operations
            .iter()
            .find(|o| o.path == "conflict")
            .unwrap()
            .action,
        Action::Conflict
    );
    let db = rusqlite::Connection::open(d.path().join("state/foldersync.sqlite3")).unwrap();
    let baseline: String = db
        .query_row(
            "SELECT digest FROM baseline WHERE task=? AND path='conflict'",
            [&t.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(baseline, blake3::hash(b"baseline").to_hex().to_string());
    assert_eq!(fs::read(t.a.join("conflict")).unwrap(), b"A edit");
    assert_eq!(fs::read(t.b.join("conflict")).unwrap(), b"B edit");
}
