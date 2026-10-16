use foldersync_engine::*;
use std::fs;
#[test]
fn durable_queue_keeps_conflict_attention_while_other_changes_sync() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let state = d.path().join("state");
    let mut e = Engine::open(&state).unwrap();
    let t = Task {
        a,
        b,
        ..Task::default()
    };
    e.save_task(&t).unwrap();
    fs::write(t.a.join("conflict"), "base").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, true).unwrap();
    fs::write(t.a.join("conflict"), "A").unwrap();
    fs::write(t.b.join("conflict"), "B").unwrap();
    e.queue_paths(&t.id, &["conflict".into()], false).unwrap();
    e.process_queue(&t.id).unwrap();
    assert_eq!(e.statuses().unwrap()[0].pending_conflicts, 1);
    fs::write(t.a.join("new"), "new content").unwrap();
    e.queue_paths(&t.id, &["new".into()], false).unwrap();
    drop(e);
    let mut e = Engine::open(&state).unwrap();
    assert_eq!(e.queued(&t.id).unwrap(), vec!["new"]);
    e.process_queue(&t.id).unwrap();
    assert_eq!(fs::read(t.b.join("new")).unwrap(), b"new content");
    assert_eq!(e.statuses().unwrap()[0].pending_conflicts, 1);
}
#[test]
fn scheduled_occurrence_survives_restart_and_coalesces_missed_runs() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let state = d.path().join("state");
    let mut e = Engine::open(&state).unwrap();
    let mut t = Task {
        a,
        b,
        ..Task::default()
    };
    t.schedule = Schedule {
        kind: "periodic".into(),
        interval_minutes: 1,
        ..Schedule::default()
    };
    e.save_task(&t).unwrap();
    assert!(e.schedule_tick(1800000000).unwrap().is_empty());
    drop(e);
    let e = Engine::open(&state).unwrap();
    assert_eq!(e.schedule_tick(1800003600).unwrap(), vec![t.id.clone()]);
    assert!(e.schedule_tick(1800003600).unwrap().is_empty());
    assert_eq!(e.queued(&t.id).unwrap(), vec!["*"]);
}
