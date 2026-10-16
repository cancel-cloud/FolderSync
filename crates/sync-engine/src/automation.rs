use crate::*;
use anyhow::Result;
use rusqlite::params;
impl Engine {
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
