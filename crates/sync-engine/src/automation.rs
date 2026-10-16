use crate::*;
use anyhow::Result;
use rusqlite::params;
impl Engine {
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
        // Background writes never imply approval for a newly destructive plan.
        if plan.operations.iter().any(|o| {
            matches!(
                o.action,
                Action::DeleteA | Action::DeleteB | Action::MoveToB
            )
        }) {
            self.index.db.execute(
                "UPDATE runs SET status='Review required' WHERE id=?",
                [&plan.id],
            )?;
            plan.status = "Review required".into();
        } else if plan
            .operations
            .iter()
            .any(|o| o.action != Action::Unchanged)
        {
            plan = self.apply(&plan.id, false)?;
        }
        self.index
            .db
            .execute("DELETE FROM dirty WHERE task=?", [id])?;
        Ok(Some(plan))
    }
}
