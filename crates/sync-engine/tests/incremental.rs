use foldersync_engine::*;
#[test]
fn one_change_only_hashes_affected_files() {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-incremental-")
        .tempdir()
        .unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    for i in 0..100 {
        std::fs::write(a.join(format!("f{i}.txt")), "base").unwrap();
    }
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    let t = Task {
        a,
        b,
        ..Task::default()
    };
    e.save_task(&t).unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    std::fs::write(t.a.join("f7.txt"), "change").unwrap();
    let p = e.preview_paths(&t.id, &["f7.txt".into()]).unwrap();
    assert_eq!(p.operations.len(), 1);
    assert_eq!(p.metrics.scanned_entries, 2);
    assert_eq!(p.metrics.hashed_bytes, 10);
    assert_eq!(p.metrics.reconciliations, 0);
    e.apply(&p.id, false).unwrap();
    assert_eq!(std::fs::read(t.b.join("f7.txt")).unwrap(), b"change");
}
