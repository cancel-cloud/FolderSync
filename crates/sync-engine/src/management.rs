use crate::{Engine, Schedule, Task, now};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::collections::BTreeSet;

struct SelectedTask {
    task: Task,
    root_a: String,
    root_b: String,
    generation: i64,
}

// Read every selected identity under the write transaction before changing any
// configuration, so stale UI selections cannot partly disable or delete tasks.
fn selected_tasks(db: &Connection, ids: &[String]) -> Result<Vec<SelectedTask>> {
    ensure!(!ids.is_empty(), "Select at least one task");
    let mut seen = BTreeSet::new();
    let mut selected = Vec::with_capacity(ids.len());
    for id in ids {
        ensure!(uuid::Uuid::parse_str(id).is_ok(), "Invalid task ID: {id}");
        ensure!(seen.insert(id), "Duplicate task selection: {id}");
        let (json, root_a, root_b, generation): (String, String, String, i64) = db
            .query_row(
                "SELECT json,root_a,root_b,generation FROM tasks WHERE id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?
            .with_context(|| format!("Task no longer exists: {id}"))?;
        selected.push(SelectedTask {
            task: serde_json::from_str(&json)?,
            root_a,
            root_b,
            generation,
        });
    }
    Ok(selected)
}

impl Engine {
    /// Disable all automatic triggers and discard queued automatic work without
    /// changing files, baseline observations, or protected versions.
    pub fn disable_tasks(&mut self, ids: &[String]) -> Result<()> {
        let transaction = self
            .index
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let selected = selected_tasks(&transaction, ids)?;
        for SelectedTask { mut task, .. } in selected {
            task.monitor = false;
            task.login = false;
            task.schedule = Schedule::default();
            transaction.execute(
                "UPDATE tasks SET json=?,generation=generation+1 WHERE id=?",
                params![serde_json::to_string(&task)?, task.id],
            )?;
            transaction.execute("DELETE FROM dirty WHERE task=?", [&task.id])?;
            for prefix in ["next/", "watch-events/"] {
                transaction.execute(
                    "DELETE FROM settings WHERE key=?",
                    [format!("{prefix}{}", task.id)],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// Remove configurations as one transaction while retaining the same
    /// protected recovery catalog and file preservation as `delete_task`.
    pub fn delete_tasks(&mut self, ids: &[String]) -> Result<()> {
        let transaction = self
            .index
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let selected = selected_tasks(&transaction, ids)?;
        for selected in &selected {
            let pending: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM displaced WHERE task=?)",
                [&selected.task.id],
                |r| r.get(0),
            )?;
            ensure!(
                !pending,
                "Protected recovery is pending for task {}; reconnect roots and restart before deleting this task",
                selected.task.id
            );
        }
        for SelectedTask {
            mut task,
            root_a,
            root_b,
            generation,
        } in selected
        {
            task.monitor = false;
            task.login = false;
            task.schedule = Schedule::default();
            task.deletion_policy.enabled = false;
            transaction.execute(
                "INSERT INTO recovery_tasks VALUES(?,?,?,?,?,?)",
                params![
                    task.id,
                    serde_json::to_string(&task)?,
                    root_a,
                    root_b,
                    generation + 1,
                    i64::try_from(now())?
                ],
            )?;
            transaction.execute("DELETE FROM dirty WHERE task=?", [&task.id])?;
            transaction.execute("DELETE FROM merkle WHERE task=?", [&task.id])?;
            for prefix in ["next/", "watch-events/", "notification/"] {
                transaction.execute(
                    "DELETE FROM settings WHERE key=?",
                    [format!("{prefix}{}", task.id)],
                )?;
            }
            transaction.execute("DELETE FROM tasks WHERE id=?", [&task.id])?;
        }
        transaction.commit()?;
        Ok(())
    }
}
