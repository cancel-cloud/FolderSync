use crate::*;
use anyhow::Result;
use rusqlite::params;
impl Engine {
    pub fn queue_watch_batch(&self, id: &str, batch: &watch::EventBatch) -> Result<()> {
        let transaction = self.index.db.unchecked_transaction()?;
        for hint in &batch.renames {
            self.record_rename_hint(id, hint)?;
        }
        self.queue_paths(id, &batch.paths, batch.uncertain)?;
        self.record_watch_events(id, batch.events)?;
        transaction.commit()?;
        Ok(())
    }
    pub fn record_watch_events(&self, id: &str, count: u64) -> Result<()> {
        let key = format!("watch-events/{id}");
        let before = self
            .setting(&key)?
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        self.set_setting(&key, &before.saturating_add(count).to_string())
    }
    pub fn pending_conflicts(&self, id: &str) -> Result<u64> {
        let count: i64 =
            self.index
                .db
                .query_row("SELECT COUNT(*) FROM conflicts WHERE task=?", [id], |r| {
                    r.get(0)
                })?;
        Ok(count as u64)
    }
    pub fn schedule_tick(&self, at: i64) -> Result<Vec<String>> {
        use chrono::TimeZone;
        let time = chrono::Local
            .timestamp_opt(at, 0)
            .single()
            .ok_or_else(|| anyhow::anyhow!("Invalid scheduler timestamp"))?;
        let mut due = vec![];
        for t in self.tasks()? {
            if t.schedule.kind.is_empty() {
                continue;
            }
            let key = format!("next/{}", t.id);
            let next = self.setting(&key)?.and_then(|s| s.parse::<i64>().ok());
            if next.is_some_and(|n| n <= at) {
                self.queue_paths(&t.id, &[], true)?;
                due.push(t.id.clone());
            }
            if (next.is_none() || next.is_some_and(|n| n <= at))
                && let Some(next) = crate::scheduler::next_due(&t.schedule, time)?
            {
                self.set_setting(&key, &next.to_string())?;
            }
        }
        Ok(due)
    }
    pub fn queue_paths(&self, id: &str, paths: &[String], uncertain: bool) -> Result<()> {
        if uncertain {
            self.index
                .db
                .execute("INSERT OR IGNORE INTO dirty VALUES(?,'*','*')", [id])?;
        }
        for path in paths {
            if !path.is_empty() {
                self.index.db.execute(
                    "INSERT OR IGNORE INTO dirty VALUES(?,'*',?)",
                    params![id, path],
                )?;
            }
        }
        Ok(())
    }
    pub fn queued(&self, id: &str) -> Result<Vec<String>> {
        let mut s = self
            .index
            .db
            .prepare("SELECT path FROM dirty WHERE task=?")?;
        let rows = s.query_map([id], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
    pub fn process_queue(&mut self, id: &str) -> Result<Option<Plan>> {
        let paths = self.queued(id)?;
        if paths.is_empty() {
            return Ok(None);
        }
        let mut plan = if paths.iter().any(|p| p == "*") {
            self.preview(id)?
        } else {
            self.preview_paths(id, &paths)?
        };
        plan.metrics.watch_events = self
            .setting(&format!("watch-events/{id}"))?
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        plan.metrics.queued_paths = paths.len() as u64;
        plan.deletion_review = self.deletion_review(&plan)?;
        let automatic_deletion = plan
            .deletion_review
            .as_ref()
            .is_some_and(|r| r.reason == DeletionReason::Approved);
        let review_required = (plan.task.conflict_policy == ConflictPolicy::RequireChoices
            && plan.operations.iter().any(|o| o.action == Action::Conflict))
            || plan
                .deletion_review
                .as_ref()
                .is_some_and(|r| r.reason != DeletionReason::Approved);
        if review_required {
            plan.status = "Review required".into();
        }
        self.index.db.execute(
            "UPDATE runs SET json=?,status=? WHERE id=?",
            params![serde_json::to_string(&plan)?, plan.status, plan.id],
        )?;
        if review_required {
            self.write_log(&plan)?;
        } else {
            // Equal observations establish/advance the common baseline and
            // clear conflicts resolved externally, even without a transfer.
            plan = match self.apply_internal(&plan.id, automatic_deletion, false) {
                Ok(plan) => plan,
                Err(error) => {
                    self.record_apply_error(&plan.id, &error)?;
                    return Err(error);
                }
            };
        }
        self.index
            .db
            .execute("DELETE FROM dirty WHERE task=?", [id])?;
        self.index.db.execute(
            "DELETE FROM settings WHERE key=?",
            [format!("watch-events/{id}")],
        )?;
        Ok(Some(plan))
    }
}

#[cfg(test)]
mod observation_tests {
    use super::*;
    use std::fs;

    fn fixture() -> (tempfile::TempDir, Engine, Task) {
        let dir = tempfile::tempdir().unwrap();
        let task = Task {
            a: dir.path().join("A"),
            b: dir.path().join("B"),
            ..Task::default()
        };
        fs::create_dir(&task.a).unwrap();
        fs::create_dir(&task.b).unwrap();
        let mut engine = Engine::open(&dir.path().join("state")).unwrap();
        engine.save_task(&task).unwrap();
        fs::write(task.a.join("equal"), "observed common content").unwrap();
        fs::write(task.b.join("equal"), "observed common content").unwrap();
        (dir, engine, task)
    }

    #[test]
    fn edit_after_equal_observation_is_not_overwritten_and_next_event_propagates_it() {
        let (_dir, mut engine, task) = fixture();
        let plan = engine.preview(&task.id).unwrap();
        fs::write(task.a.join("equal"), "edit after observation").unwrap();
        engine.apply_internal(&plan.id, false, false).unwrap();
        assert_eq!(
            fs::read(task.a.join("equal")).unwrap(),
            b"edit after observation"
        );
        assert_eq!(
            fs::read(task.b.join("equal")).unwrap(),
            b"observed common content"
        );
        engine
            .queue_paths(&task.id, &["equal".into()], false)
            .unwrap();
        let applied = engine.process_queue(&task.id).unwrap().unwrap();
        assert_eq!(applied.operations[0].action, Action::CopyToB);
        assert_eq!(applied.metrics.reconciliations, 0);
        assert_eq!(
            fs::read(task.b.join("equal")).unwrap(),
            b"edit after observation"
        );
    }

    #[test]
    fn manual_equal_plan_still_rejects_a_stale_observation() {
        let (_dir, mut engine, task) = fixture();
        let plan = engine.preview(&task.id).unwrap();
        fs::write(task.a.join("equal"), "new content").unwrap();
        assert!(
            engine
                .apply(&plan.id, false)
                .unwrap_err()
                .to_string()
                .contains("Stale input")
        );
        assert_eq!(
            fs::read(task.b.join("equal")).unwrap(),
            b"observed common content"
        );
    }

    #[test]
    fn automatic_mutating_plan_keeps_global_stale_input_barrier() {
        let (_dir, mut engine, task) = fixture();
        fs::write(task.a.join("new"), "must not be copied from stale plan").unwrap();
        let plan = engine.preview(&task.id).unwrap();
        fs::write(task.a.join("equal"), "changed unrelated reviewed input").unwrap();
        assert!(
            engine
                .apply_internal(&plan.id, false, false)
                .unwrap_err()
                .to_string()
                .contains("Stale input")
        );
        assert!(!task.b.join("new").exists());
    }

    #[test]
    fn automatic_equal_plan_still_rejects_changed_root_identity() {
        let (dir, mut engine, task) = fixture();
        let plan = engine.preview(&task.id).unwrap();
        fs::rename(&task.a, dir.path().join("old-A")).unwrap();
        fs::create_dir(&task.a).unwrap();
        assert!(engine.apply_internal(&plan.id, false, false).is_err());
        assert_eq!(
            fs::read(task.b.join("equal")).unwrap(),
            b"observed common content"
        );
    }

    #[cfg(windows)]
    #[test]
    fn failed_incremental_discovery_does_not_commit_partial_observations() {
        use std::os::windows::fs::OpenOptionsExt;
        let (_dir, mut engine, task) = fixture();
        fs::write(task.a.join("z-locked"), "readable before lock").unwrap();
        engine.preview(&task.id).unwrap();
        let before = engine.index.entry(&task.id, "A", "equal").unwrap();
        fs::write(task.a.join("equal"), "new observation before later failure").unwrap();
        let _lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(task.a.join("z-locked"))
            .unwrap();
        assert!(
            engine
                .preview_paths(&task.id, &["equal".into(), "z-locked".into()])
                .is_err()
        );
        assert_eq!(
            engine.index.entry(&task.id, "A", "equal").unwrap(),
            before,
            "Failed discovery must retain the previous coherent observations"
        );
    }
}
