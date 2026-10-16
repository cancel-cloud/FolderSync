use foldersync_engine::*;
use std::fs;
#[test]
fn configured_history_cap_retains_verified_recent_versions() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    let t = Task {
        a,
        b,
        history_patterns: vec!["*.txt".into()],
        history_cap: 2,
        ..Task::default()
    };
    e.save_task(&t).unwrap();
    for i in 0..5 {
        fs::write(t.a.join("history.txt"), format!("version {i}")).unwrap();
        let p = e.preview(&t.id).unwrap();
        e.apply(&p.id, true).unwrap();
    }
    let versions = e.versions(&t.id).unwrap();
    assert_eq!(versions.len(), 2);
    let contents: Vec<_> = versions
        .iter()
        .map(|v| fs::read_to_string(&v.stored).unwrap())
        .collect();
    assert!(contents.contains(&"version 3".into()));
    assert!(contents.contains(&"version 2".into()));
    assert_eq!(
        fs::read_to_string(t.b.join("history.txt")).unwrap(),
        "version 4"
    );
}
