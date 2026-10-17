use crate::*;
use anyhow::{Result, ensure};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ManualReview {
    run: String,
    mode: Mode,
    scope: Scope,
    root_a: String,
    root_b: String,
}
pub(crate) fn manual_review_json(p: &Plan) -> Result<String> {
    Ok(serde_json::to_string(&ManualReview {
        run: p.id.clone(),
        mode: p.task.mode,
        scope: p.task.scope.clone(),
        root_a: p.root_a.clone(),
        root_b: p.root_b.clone(),
    })?)
}
impl Engine {
    pub(crate) fn deletion_baseline_run(&self, t: &Task) -> Result<Option<String>> {
        let (a, b) = self.index.roots(&t.id)?;
        Ok(self.approved_baseline(t, &a, &b)?.map(|r| r.run))
    }
    fn approved_baseline(
        &self,
        t: &Task,
        root_a: &str,
        root_b: &str,
    ) -> Result<Option<ManualReview>> {
        let value: Option<String> = self
            .index
            .db
            .query_row(
                "SELECT json FROM baseline_reviews WHERE task=?",
                [&t.id],
                |r| r.get(0),
            )
            .optional()?;
        let review = value
            .map(|v| serde_json::from_str::<ManualReview>(&v))
            .transpose()?;
        Ok(review.filter(|r| {
            r.mode == t.mode && r.scope == t.scope && r.root_a == root_a && r.root_b == root_b
        }))
    }
    pub(crate) fn validate_deletion_policy(&self, t: &Task) -> Result<()> {
        let policy = &t.deletion_policy;
        ensure!(
            (1..=100_000).contains(&policy.max_entries) && (1..=100).contains(&policy.max_percent),
            "Invalid automatic deletion limits"
        );
        if policy.enabled {
            ensure!(
                matches!(t.mode, Mode::TwoWay | Mode::Mirror),
                "Automatic deletion is available only for two-way and mirror tasks; Move requires review"
            );
            // A selected policy may be saved before the first manual baseline.
            // Execution remains gated by approved_baseline in deletion_review.
        }
        Ok(())
    }
    pub(crate) fn deletion_review(&self, p: &Plan) -> Result<Option<DeletionReview>> {
        let removals = p
            .operations
            .iter()
            .filter(|o| {
                matches!(
                    o.action,
                    Action::DeleteA | Action::DeleteB | Action::MoveToB
                )
            })
            .collect::<Vec<_>>();
        if removals.is_empty() {
            return Ok(None);
        }
        let policy = &p.task.deletion_policy;
        let baseline_entries:i64=self.index.db.query_row("SELECT COUNT(*) FROM baseline b WHERE b.task=? AND b.digest IS NOT NULL AND EXISTS(SELECT 1 FROM entries e WHERE e.task=b.task AND e.path=b.path)",[&p.task.id],|r|r.get(0))?;
        let approval = self.approved_baseline(&p.task, &p.root_a, &p.root_b)?;
        let mut review = DeletionReview {
            reason: DeletionReason::Approved,
            entries: removals.len() as u64,
            baseline_entries: baseline_entries as u64,
            max_entries: policy.max_entries,
            max_percent: policy.max_percent,
            baseline_run: approval.as_ref().map(|r| r.run.clone()),
        };
        review.reason = if removals.iter().any(|o| o.action == Action::MoveToB) {
            DeletionReason::MoveRequiresReview
        } else if !policy.enabled {
            DeletionReason::Disabled
        } else if approval.is_none() {
            DeletionReason::BaselineApprovalRequired
        } else {
            let mut proven = true;
            for o in removals {
                let digest: Option<Option<String>> = self
                    .index
                    .db
                    .query_row(
                        "SELECT digest FROM baseline WHERE task=? AND path=?",
                        params![p.task.id, crate::fs::path_key(&o.path)],
                        |r| r.get(0),
                    )
                    .optional()?;
                let expected = match o.action {
                    Action::DeleteA => o.a.as_ref(),
                    Action::DeleteB => o.b.as_ref(),
                    _ => None,
                };
                if digest.flatten().as_deref() != expected.map(|e| e.digest.as_str())
                    || expected.is_none()
                {
                    proven = false;
                    break;
                }
            }
            if !proven {
                DeletionReason::UnprovenDeletion
            } else if review.entries > u64::from(policy.max_entries) {
                DeletionReason::EntryLimit
            } else if review.baseline_entries == 0
                || review.entries.saturating_mul(100)
                    > review
                        .baseline_entries
                        .saturating_mul(u64::from(policy.max_percent))
            {
                DeletionReason::PercentLimit
            } else {
                DeletionReason::Approved
            }
        };
        Ok(Some(review))
    }
}
