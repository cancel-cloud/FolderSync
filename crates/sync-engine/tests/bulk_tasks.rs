use foldersync_engine::*;
use rusqlite::{Connection, params};
use std::fs;

fn fixture() -> (tempfile::TempDir, Engine, Vec<Task>) {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-bulk-tasks-")
        .tempdir()
        .unwrap();
    let mut engine = Engine::open(&d.path().join("state")).unwrap();
    let tasks = (0..3)
        .map(|n| {
            let task = Task {
                a: d.path().join(format!("A{n}")),
                b: d.path().join(format!("B{n}")),
                monitor: true,
                login: true,
                schedule: Schedule {
                    kind: "periodic".into(),
                    interval_minutes: 1,
                    ..Schedule::default()
                },
                ..Task::default()
            };
            fs::create_dir(&task.a).unwrap();
            fs::create_dir(&task.b).unwrap();
            engine.save_task(&task).unwrap();
            fs::write(task.a.join("f.txt"), "original").unwrap();
            let p = engine.preview(&task.id).unwrap();
            engine.apply(&p.id, false).unwrap();
            fs::write(task.a.join("f.txt"), "current").unwrap();
            let p = engine.preview(&task.id).unwrap();
            engine.apply(&p.id, false).unwrap();
            engine
                .queue_paths(&task.id, &["f.txt".into()], true)
                .unwrap();
            engine
                .set_setting(&format!("next/{}", task.id), "1800000000")
                .unwrap();
            engine.record_watch_events(&task.id, 3).unwrap();
            task
        })
        .collect();
    (d, engine, tasks)
}

fn db(d: &tempfile::TempDir) -> Connection {
    Connection::open(d.path().join("state/foldersync.sqlite3")).unwrap()
}

#[test]
fn bulk_selection_rejects_empty_duplicate_invalid_and_stale_ids_without_partial_changes() {
    let (_d, mut e, tasks) = fixture();
    for ids in [
        vec![],
        vec![tasks[0].id.clone(), tasks[0].id.clone()],
        vec![tasks[0].id.clone(), "invalid".into()],
        vec![tasks[0].id.clone(), uuid::Uuid::new_v4().to_string()],
        vec![tasks[0].identifier.clone()],
    ] {
        assert!(e.disable_tasks(&ids).is_err());
        assert!(e.delete_tasks(&ids).is_err());
        for t in &tasks {
            assert_eq!(
                e.tasks()
                    .unwrap()
                    .into_iter()
                    .find(|x| x.id == t.id)
                    .unwrap(),
                *t
            );
            assert!(!e.queued(&t.id).unwrap().is_empty());
        }
        assert!(e.recovery_tasks().unwrap().is_empty());
    }
}

#[test]
fn bulk_disable_survives_restart_clears_automation_and_preserves_files_versions_and_baselines() {
    let (d, mut e, tasks) = fixture();
    let baseline: Vec<(String, String, Option<String>)> = db(&d)
        .prepare("SELECT task,path,digest FROM baseline ORDER BY task,path")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let stale = e.preview(&tasks[0].id).unwrap();
    e.disable_tasks(&tasks[..2].iter().map(|t| t.id.clone()).collect::<Vec<_>>())
        .unwrap();
    assert!(e.apply(&stale.id, false).is_err());
    drop(e);
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    let after: Vec<(String, String, Option<String>)> = db(&d)
        .prepare("SELECT task,path,digest FROM baseline ORDER BY task,path")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(after, baseline);
    let due = e.schedule_tick(1800003600).unwrap();
    assert_eq!(due, vec![tasks[2].id.clone()]);
    for t in &tasks[..2] {
        let mut expected = t.clone();
        expected.monitor = false;
        expected.login = false;
        expected.schedule = Schedule::default();
        assert_eq!(
            e.tasks()
                .unwrap()
                .into_iter()
                .find(|x| x.id == t.id)
                .unwrap(),
            expected
        );
        assert!(e.queued(&t.id).unwrap().is_empty());
        assert!(e.process_queue(&t.id).unwrap().is_none());
        assert!(e.setting(&format!("next/{}", t.id)).unwrap().is_none());
        assert!(
            e.setting(&format!("watch-events/{}", t.id))
                .unwrap()
                .is_none()
        );
        assert_eq!(fs::read(t.a.join("f.txt")).unwrap(), b"current");
        assert_eq!(fs::read(t.b.join("f.txt")).unwrap(), b"current");
        assert_eq!(e.versions(&t.id).unwrap().len(), 1);
    }
    assert_eq!(
        e.tasks()
            .unwrap()
            .into_iter()
            .find(|x| x.id == tasks[2].id)
            .unwrap(),
        tasks[2]
    );
}

#[test]
fn bulk_delete_rejects_pending_recovery_before_deleting_any_task_then_retains_restorable_versions()
{
    let (d, mut e, tasks) = fixture();
    let ids = tasks[..2].iter().map(|t| t.id.clone()).collect::<Vec<_>>();
    let displaced = tasks[1].b.join(".foldersync-displaced-owned-test");
    fs::write(&displaced, "protected").unwrap();
    db(&d)
        .execute(
            "INSERT INTO displaced VALUES(?,?,?,?)",
            params![displaced.to_str().unwrap(), tasks[1].id, "f.txt", "B"],
        )
        .unwrap();
    assert!(
        e.delete_tasks(&ids)
            .unwrap_err()
            .to_string()
            .contains("recovery")
    );
    assert_eq!(e.tasks().unwrap().len(), 3);
    assert!(e.recovery_tasks().unwrap().is_empty());
    assert_eq!(fs::read(&displaced).unwrap(), b"protected");
    db(&d).execute("DELETE FROM displaced", []).unwrap();
    e.delete_tasks(&ids).unwrap();
    assert_eq!(e.tasks().unwrap(), vec![tasks[2].clone()]);
    assert_eq!(e.recovery_tasks().unwrap().len(), 2);
    drop(e);
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    for t in &tasks[..2] {
        assert!(e.queued(&t.id).unwrap().is_empty());
        assert_eq!(fs::read(t.b.join("f.txt")).unwrap(), b"current");
        let v = e.versions(&t.id).unwrap().remove(0);
        let p = e.restore_preview(&v.id).unwrap();
        assert!(p.archived);
        e.restore_apply(&p.id).unwrap();
        assert_eq!(fs::read(t.b.join("f.txt")).unwrap(), b"original");
        assert_eq!(fs::read(t.a.join("f.txt")).unwrap(), b"current");
    }
}

#[test]
fn bulk_mutation_database_errors_roll_back_earlier_selected_tasks() {
    let (d, mut e, tasks) = fixture();
    let ids = tasks[..2].iter().map(|t| t.id.clone()).collect::<Vec<_>>();
    db(&d).execute_batch(&format!("CREATE TRIGGER reject_disable BEFORE UPDATE ON tasks WHEN OLD.id='{}' BEGIN SELECT RAISE(ABORT,'owned test rejection'); END; CREATE TRIGGER reject_delete BEFORE DELETE ON tasks WHEN OLD.id='{}' BEGIN SELECT RAISE(ABORT,'owned test rejection'); END;", tasks[1].id, tasks[1].id)).unwrap();
    assert!(e.disable_tasks(&ids).is_err());
    assert!(e.delete_tasks(&ids).is_err());
    for t in &tasks {
        assert_eq!(
            e.tasks()
                .unwrap()
                .into_iter()
                .find(|x| x.id == t.id)
                .unwrap(),
            *t
        );
        assert!(!e.queued(&t.id).unwrap().is_empty());
    }
    assert!(e.recovery_tasks().unwrap().is_empty());
}
