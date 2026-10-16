use crate::*;
use anyhow::Result;
use rusqlite::params;

impl From<&Plan> for PlanHeader {
    fn from(p: &Plan) -> Self {
        let mut summary = PlanSummary::default();
        for o in &p.operations {
            summary.entries += 1;
            match o.action {
                Action::CopyToA => {
                    summary.copies += 1;
                    summary.to_a += 1;
                }
                Action::CopyToB => {
                    summary.copies += 1;
                    summary.to_b += 1;
                }
                Action::MoveToB => {
                    summary.copies += 1;
                    summary.to_b += 1;
                    summary.deletes += 1;
                    summary.remove_a += 1;
                }
                Action::DeleteA => {
                    summary.deletes += 1;
                    summary.remove_a += 1;
                }
                Action::DeleteB => {
                    summary.deletes += 1;
                    summary.remove_b += 1;
                }
                Action::Conflict => summary.conflicts += 1,
                Action::Unchanged => summary.unchanged += 1,
            }
            summary.explicit |= o.reason.starts_with("Explicit");
        }
        Self {
            id: p.id.clone(),
            task: p.task.clone(),
            metrics: p.metrics.clone(),
            created_at: p.created_at,
            status: p.status.clone(),
            summary,
        }
    }
}
impl Engine {
    pub fn plan_header(&self, id: &str) -> Result<PlanHeader> {
        // SQLite removes the large array before it crosses into Rust/IPC.
        let (json, status): (String, String) = self.index.db.query_row(
            "SELECT json_remove(json,'$.operations'),status FROM runs WHERE id=?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let mut header: PlanHeader = serde_json::from_str(&json)?;
        header.status = status;
        header.summary = self.index.db.query_row(
            "SELECT coalesce(sum(a IN ('CopyToA','CopyToB','MoveToB')),0),coalesce(sum(a IN ('DeleteA','DeleteB','MoveToB')),0),coalesce(sum(a='Conflict'),0),coalesce(sum(a='Unchanged'),0),coalesce(sum(a='CopyToA'),0),coalesce(sum(a IN ('CopyToB','MoveToB')),0),coalesce(sum(a IN ('DeleteA','MoveToB')),0),coalesce(sum(a='DeleteB'),0),coalesce(max(reason LIKE 'Explicit%'),0),count(*) FROM (SELECT json_extract(value,'$.action') a,json_extract(value,'$.reason') reason FROM runs,json_each(runs.json,'$.operations') WHERE runs.id=?)", [id], |r| Ok(PlanSummary { copies:r.get::<_,i64>(0)? as u64, deletes:r.get::<_,i64>(1)? as u64, conflicts:r.get::<_,i64>(2)? as u64, unchanged:r.get::<_,i64>(3)? as u64, to_a:r.get::<_,i64>(4)? as u64, to_b:r.get::<_,i64>(5)? as u64, remove_a:r.get::<_,i64>(6)? as u64, remove_b:r.get::<_,i64>(7)? as u64, explicit:r.get(8)?, entries:r.get::<_,i64>(9)? as u64 }))?;
        Ok(header)
    }
    pub fn history_headers(&self, task: &str) -> Result<Vec<PlanHeader>> {
        let mut statement = self.index.db.prepare(
            "SELECT id FROM runs WHERE task=? ORDER BY created DESC,rowid DESC LIMIT 100",
        )?;
        let ids = statement.query_map([task], |r| r.get::<_, String>(0))?;
        ids.map(|id| self.plan_header(&id?)).collect()
    }
    pub fn operation_page(&self, id: &str, q: &OperationQuery) -> Result<OperationPage> {
        // Check existence even when the page/filter matches zero rows.
        let _: i64 = self
            .index
            .db
            .query_row("SELECT 1 FROM runs WHERE id=?", [id], |r| r.get(0))?;
        let filter = q.filter.to_lowercase();
        let predicate = " FROM runs,json_each(runs.json,'$.operations') WHERE runs.id=?1 AND (?2 OR json_extract(value,'$.action')!='Unchanged') AND (?3='' OR instr(foldersync_fold(json_extract(value,'$.path')),?3)>0)";
        let total: i64 = self.index.db.query_row(
            &format!("SELECT count(*){predicate}"),
            params![id, q.show_unchanged, filter],
            |r| r.get(0),
        )?;
        // Only fixed trusted SQL fragments are interpolated. Search text is bound.
        let order = if q.sort == "action" {
            "json_extract(value,'$.action'),"
        } else {
            ""
        };
        let mut statement = self.index.db.prepare(&format!("SELECT value{predicate} ORDER BY {order}json_extract(value,'$.path') COLLATE NOCASE,json_extract(value,'$.path') LIMIT ?4 OFFSET ?5"))?;
        let rows = statement.query_map(
            params![
                id,
                q.show_unchanged,
                filter,
                q.limit.clamp(1, 200),
                q.offset
            ],
            |r| r.get::<_, String>(0),
        )?;
        let operations = rows
            .map(|r| Ok(serde_json::from_str(&r?)?))
            .collect::<Result<_>>()?;
        Ok(OperationPage {
            operations,
            total: total as u64,
        })
    }
}
