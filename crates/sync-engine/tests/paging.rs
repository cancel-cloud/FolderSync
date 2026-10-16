use foldersync_engine::*;
use std::fs;

#[test]
fn review_pages_are_bounded_filtered_and_history_has_no_operation_payload() {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-pages-")
        .tempdir()
        .unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    for i in 0..405 {
        fs::write(a.join(format!("file-{i:04}.txt")), "page fixture").unwrap();
    }
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    let t = Task {
        a,
        b,
        ..Task::default()
    };
    e.save_task(&t).unwrap();
    let p = e.preview(&t.id).unwrap();
    let headers = e.history_headers(&t.id).unwrap();
    assert_eq!(headers[0].summary.copies, 405);
    assert_eq!(headers[0].summary.to_b, 405);
    assert_eq!(headers[0].summary.entries, 405);
    let serialized = serde_json::to_string(&headers).unwrap();
    assert!(!serialized.contains("operations"));
    assert!(!serialized.contains("file-0000.txt"));
    let request = OperationQuery {
        offset: 200,
        limit: 99999,
        ..Default::default()
    };
    let page = e.operation_page(&p.id, &request).unwrap();
    assert_eq!(page.total, 405);
    assert_eq!(page.operations.len(), 200);
    assert_eq!(page.operations[0].path, "file-0200.txt");
    let filtered = e
        .operation_page(
            &p.id,
            &OperationQuery {
                filter: "FILE-040".into(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(filtered.total, 5);
    assert_eq!(filtered.operations.len(), 5);
    assert_eq!(fs::read_dir(&t.b).unwrap().count(), 0);
    let applied = e.apply(&p.id, false).unwrap();
    assert_eq!(e.plan_header(&p.id).unwrap().status, applied.status);
    assert_eq!(e.statuses().unwrap()[0].last_run.as_ref().unwrap().id, p.id);
    fs::write(t.a.join("Änderung.txt"), "Unicode filter fixture").unwrap();
    let unicode = e.preview(&t.id).unwrap();
    for filter in ["Änderung", "änderung"] {
        let page = e
            .operation_page(
                &unicode.id,
                &OperationQuery {
                    filter: filter.into(),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(page.total, 1, "Unicode filter {filter}");
        assert_eq!(page.operations[0].path, "Änderung.txt");
    }
}

#[test]
fn ten_thousand_persisted_operations_have_small_headers_and_consistent_counts() {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-large-plan-")
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
    let mut p = e.preview(&t.id).unwrap();
    let actions = [
        Action::CopyToA,
        Action::CopyToB,
        Action::DeleteA,
        Action::DeleteB,
        Action::MoveToB,
        Action::Conflict,
        Action::Unchanged,
    ];
    p.operations = (0..10_000)
        .map(|i| Operation {
            path: format!("row-{i:05}"),
            action: actions[i % actions.len()],
            reason: "Explicit fixture choice".into(),
            status: "Planned".into(),
            a: None,
            b: None,
        })
        .collect();
    // Storage/IPC contract fixture; no executor uses these synthetic operations.
    let db = rusqlite::Connection::open(e.storage().join("foldersync.sqlite3")).unwrap();
    db.execute(
        "UPDATE runs SET json=?,status='Interrupted' WHERE id=?",
        rusqlite::params![serde_json::to_string(&p).unwrap(), p.id],
    )
    .unwrap();
    let header = e.plan_header(&p.id).unwrap();
    assert_eq!(header.status, "Interrupted");
    assert_eq!(
        serde_json::to_value(&header.summary).unwrap(),
        serde_json::to_value(PlanHeader::from(&p).summary).unwrap()
    );
    assert!(serde_json::to_vec(&header).unwrap().len() < 4096);
    let page = e
        .operation_page(
            &p.id,
            &OperationQuery {
                show_unchanged: true,
                offset: 9800,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(page.total, 10_000);
    assert_eq!(page.operations.len(), 200);
    assert_eq!(page.operations[199].path, "row-09999");
    let hidden = e.operation_page(&p.id, &OperationQuery::default()).unwrap();
    assert_eq!(hidden.total, 10_000 - header.summary.unchanged);
    let literal = e
        .operation_page(
            &p.id,
            &OperationQuery {
                filter: "' OR 1=1 --".into(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(literal.total, 0);
    assert!(
        e.operation_page("absent", &OperationQuery::default())
            .is_err()
    );
}
