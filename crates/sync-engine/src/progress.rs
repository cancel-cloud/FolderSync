use crate::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgressPhase {
    Discovering,
    Validating,
    Applying,
    Transferring,
    Protecting,
    Verifying,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunProgress {
    pub task_id: String,
    pub run_id: Option<String>,
    pub phase: ProgressPhase,
    pub completed_entries: u64,
    pub total_entries: u64,
    pub path: Option<String>,
    pub file_bytes: u64,
    pub file_total: u64,
    #[serde(default)]
    pub active_transfers: u32,
    #[serde(default)]
    pub max_transfers: u32,
}
/// A single replaceable snapshot, independent of the engine/database mutex.
#[derive(Clone, Default)]
pub struct ProgressState(Arc<Mutex<Option<RunProgress>>>);
impl ProgressState {
    pub fn snapshot(&self) -> Option<RunProgress> {
        self.0.lock().ok()?.clone()
    }
    pub(crate) fn begin(
        &self,
        task_id: &str,
        run_id: Option<&str>,
        phase: ProgressPhase,
        total: usize,
    ) -> ProgressGuard {
        if let Ok(mut value) = self.0.lock() {
            *value = Some(RunProgress {
                task_id: task_id.into(),
                run_id: run_id.map(str::to_owned),
                phase,
                completed_entries: 0,
                total_entries: total as u64,
                path: None,
                file_bytes: 0,
                file_total: 0,
                active_transfers: 0,
                max_transfers: 1,
            });
        }
        ProgressGuard(self.clone())
    }
    pub(crate) fn update(&self, update: impl FnOnce(&mut RunProgress)) {
        if let Ok(mut value) = self.0.lock()
            && let Some(value) = value.as_mut()
        {
            update(value);
        }
    }
    pub(crate) fn hashed_file(&self, path: &std::path::Path, bytes: u64, total: u64) {
        self.update(|state| {
            if state.phase == ProgressPhase::Applying {
                state.phase = ProgressPhase::Validating;
            }
            if matches!(
                state.phase,
                ProgressPhase::Discovering | ProgressPhase::Validating
            ) {
                state.path = Some(path.to_string_lossy().into_owned());
            }
            // One atomic snapshot: concurrent readers must never mix one
            // file's byte count with a different file's total or display path.
            state.file_bytes = bytes;
            state.file_total = total;
        });
    }
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    #[test]
    fn interleaved_hash_readers_publish_one_coherent_file_snapshot() {
        let state = ProgressState::default();
        let _guard = state.begin("task", None, ProgressPhase::Discovering, 2);
        let a = std::path::Path::new("large.bin");
        let b = std::path::Path::new("small.bin");
        state.hashed_file(a, 4096, 8192);
        state.hashed_file(b, 128, 256);
        state.hashed_file(a, 8192, 8192);
        let snapshot = state.snapshot().unwrap();
        assert_eq!(snapshot.path.as_deref(), Some("large.bin"));
        assert_eq!((snapshot.file_bytes, snapshot.file_total), (8192, 8192));
        state.update(|s| {
            s.phase = ProgressPhase::Verifying;
            s.path = Some("reviewed-name.bin".into());
        });
        state.hashed_file(std::path::Path::new(".foldersync-stage-internal"), 256, 256);
        assert_eq!(
            state.snapshot().unwrap().path.as_deref(),
            Some("reviewed-name.bin")
        );
    }
}
pub(crate) struct ProgressGuard(ProgressState);
impl Drop for ProgressGuard {
    fn drop(&mut self) {
        if let Ok(mut value) = self.0.0.lock() {
            *value = None;
        }
    }
}
#[derive(Debug)]
pub struct Cancelled;
impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cancelled at a safe boundary; completed operations remain committed")
    }
}
impl std::error::Error for Cancelled {}
pub(crate) fn check_cancel(cancel: Option<&AtomicBool>) -> Result<()> {
    if cancel.is_some_and(|value| value.load(Ordering::Relaxed)) {
        Err(Cancelled.into())
    } else {
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApplyOutcome {
    pub plan: PlanHeader,
    pub error: Option<String>,
}
impl Engine {
    pub fn apply_reviewed(&mut self, id: &str, destructive: bool) -> Result<ApplyOutcome> {
        match self.apply(id, destructive) {
            Ok(plan) => Ok(ApplyOutcome {
                plan: PlanHeader::from(&plan),
                error: None,
            }),
            Err(error) => {
                let plan = self.record_apply_error(id, &error)?;
                Ok(ApplyOutcome {
                    plan: PlanHeader::from(&plan),
                    error: Some(format!("{error:#}")),
                })
            }
        }
    }
    pub(crate) fn record_apply_error(&self, id: &str, error: &anyhow::Error) -> Result<Plan> {
        let mut plan = self.index.plan(id)?;
        if matches!(plan.status.as_str(), "Preview" | "Review required") {
            // Validation is an attempted run, but it must not consume or
            // mutate the reviewed plan. Save a distinct terminal attempt.
            plan.id = uuid::Uuid::new_v4().to_string();
            plan.created_at = now();
            plan.status = if error.is::<Cancelled>() {
                "Cancelled".into()
            } else {
                format!("Rejected: {error:#}")
            };
            for operation in &mut plan.operations {
                operation.status = "Not run".into();
            }
            self.index.put_plan(&plan)?;
            self.write_log(&plan)?;
        } else if plan.status == "Applying" {
            // A database/pre-executor error can leave Applying behind.
            plan.status = format!("Failed: {error:#}");
            self.index.db.execute(
                "UPDATE runs SET status=? WHERE id=?",
                rusqlite::params![plan.status, plan.id],
            )?;
            self.write_log(&plan)?;
        }
        Ok(plan)
    }
    pub(crate) fn record_discovery_error(
        &self,
        task: &Task,
        metrics: &Metrics,
        error: &anyhow::Error,
    ) -> Result<()> {
        let (root_a, root_b) = self.index.roots(&task.id)?;
        let plan = Plan {
            id: uuid::Uuid::new_v4().to_string(),
            task: task.clone(),
            root_a,
            root_b,
            operations: vec![],
            metrics: metrics.clone(),
            baseline_generation: self.index.generation(&task.id)?,
            created_at: now(),
            status: if error.is::<Cancelled>() {
                "Cancelled".into()
            } else {
                format!("Failed: Discovery: {error:#}")
            },
            deletion_review: None,
        };
        self.index.put_plan(&plan)?;
        self.write_log(&plan)
    }
    pub(crate) fn read_entry(
        &self,
        root: &std::path::Path,
        relative: &str,
        scope: &Scope,
        metrics: &mut Metrics,
    ) -> Result<Option<Entry>> {
        crate::fs::read_entry_controlled(
            root,
            relative,
            scope,
            metrics,
            Some(&self.cancel),
            Some(&self.progress),
        )
    }
}
