use foldersync_engine::*;
use std::fs;

#[test]
fn full_parallel_scan_preserves_filters_digests_and_exact_metrics() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("source");
    fs::create_dir_all(root.join("empty")).unwrap();
    let mut expected_bytes = 0;
    for i in 0..256 {
        let dir = root.join(format!("Projekt-東京-{}", i / 16));
        fs::create_dir_all(&dir).unwrap();
        let bytes = vec![i as u8; 128 + i * 31];
        fs::write(dir.join(format!("{i}.dat")), &bytes).unwrap();
        fs::write(dir.join(format!("{i}.tmp")), b"excluded").unwrap();
        expected_bytes += bytes.len() as u64;
    }
    let scope = Scope {
        exclude: vec!["*.tmp".into()],
        ..Scope::default()
    };
    let mut metrics = Metrics::default();
    let tree = foldersync_engine::fs::scan(&root, &scope, &mut metrics).unwrap();
    assert_eq!(tree.len(), 256 + 16 + 1);
    assert_eq!(metrics.scanned_entries, tree.len() as u64);
    assert_eq!(metrics.hashed_bytes, expected_bytes);
    for entry in tree.values().filter(|entry| !entry.directory) {
        assert_eq!(
            foldersync_engine::fs::hash_file(&root.join(&entry.path), &mut Metrics::default())
                .unwrap()
                .0,
            entry.digest
        );
    }
}

#[test]
fn stale_input_in_parallel_validation_prevents_all_mutations() {
    let fixture = tempfile::tempdir().unwrap();
    let task = Task {
        a: fixture.path().join("A"),
        b: fixture.path().join("B"),
        ..Task::default()
    };
    fs::create_dir(&task.a).unwrap();
    fs::create_dir(&task.b).unwrap();
    for i in 0..128 {
        fs::write(task.a.join(format!("{i:03}.dat")), vec![i as u8; 1024]).unwrap();
    }
    let mut engine = Engine::open(&fixture.path().join("state")).unwrap();
    engine.save_task(&task).unwrap();
    let plan = engine.preview(&task.id).unwrap();
    fs::write(task.a.join("064.dat"), b"changed after review").unwrap();
    assert!(
        engine
            .apply(&plan.id, false)
            .unwrap_err()
            .to_string()
            .contains("Stale input")
    );
    assert_eq!(fs::read_dir(&task.b).unwrap().count(), 0);
    let db = rusqlite::Connection::open(fixture.path().join("state/foldersync.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM journal", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[cfg(windows)]
#[test]
fn one_locked_input_rejects_the_entire_parallel_scan() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = tempfile::tempdir().unwrap();
    for i in 0..128 {
        fs::write(fixture.path().join(format!("{i:03}.dat")), b"original").unwrap();
    }
    let _locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(fixture.path().join("064.dat"))
        .unwrap();
    assert!(
        foldersync_engine::fs::scan(fixture.path(), &Scope::default(), &mut Metrics::default())
            .is_err()
    );
}
