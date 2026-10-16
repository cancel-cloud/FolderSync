//! File identities are candidate links, never authority to mutate a path.
//! Every connected path is re-read through the normal scope/root safeguards.
use crate::{fs::path_key, index::Index, *};
use anyhow::Result;
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(crate) const LINEAGE_RENAME: &str =
    "Rename identity connects divergent paths; originals preserved until explicit review";

impl Index {
    fn link_candidate(&self, task: &str, origin: &str, target: &str) -> Result<()> {
        let digest:String=self.db.query_row("SELECT COALESCE((SELECT json_extract(json,'$.digest') FROM baseline_identity WHERE task=? AND path=? LIMIT 1),(SELECT digest FROM rename_links WHERE task=? AND target=? LIMIT 1),'')",params![task,origin,task,origin],|r|r.get(0))?;
        // Invalidate first: a crash may invalidate an extra review, but never
        // leaves a new link with an older executable plan.
        self.db.execute("UPDATE tasks SET generation=generation+1 WHERE id=? AND NOT EXISTS(SELECT 1 FROM rename_links WHERE task=? AND origin=? AND target=?)",params![task,task,origin,target])?;
        self.db.execute(
            "INSERT OR IGNORE INTO rename_links VALUES(?,?,?,?)",
            params![task, origin, target, digest],
        )?;
        Ok(())
    }
    pub(crate) fn remember_common(
        db: &Connection,
        task: &str,
        path: &str,
        a: Option<&Entry>,
        b: Option<&Entry>,
    ) -> Result<()> {
        if let Some((a, b)) = a
            .zip(b)
            .filter(|(a, b)| !a.directory && !b.directory && a.digest == b.digest)
        {
            for (side, e) in [("A", a), ("B", b)] {
                db.execute(
                    "INSERT OR REPLACE INTO baseline_identity VALUES(?,?,?,?,?)",
                    params![
                        task,
                        side,
                        path,
                        e.file_id.as_deref().unwrap_or(""),
                        serde_json::to_string(e)?
                    ],
                )?;
            }
        }
        // Keep the last common identities after an automatic absence observation.
        // A delayed target notification may still identify a renamed original.
        Ok(())
    }
    fn identity_origins(&self, task: &str, side: &str, id: &str) -> Result<Vec<String>> {
        let mut s = self
            .db
            .prepare("SELECT path FROM baseline_identity WHERE task=? AND side=? AND file_id=?")?;
        Ok(s.query_map(params![task, side, id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }
    fn linked(&self, task: &str, path: &str) -> Result<Vec<String>> {
        let mut s = self.db.prepare(
            "SELECT origin,target FROM rename_links WHERE task=? AND (origin=? OR target=?)",
        )?;
        let rows = s.query_map(params![task, path, path], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut paths = vec![];
        for row in rows {
            let (a, b) = row?;
            paths.extend([a, b]);
        }
        Ok(paths)
    }
    fn content_origins(&self, task: &str, path: &str, digest: &str) -> Result<Vec<String>> {
        let mut s=self.db.prepare("SELECT DISTINCT origin FROM rename_links WHERE task=? AND digest=? AND NOT EXISTS(SELECT 1 FROM baseline WHERE task=? AND path=? AND digest IS NOT NULL)")?;
        Ok(
            s.query_map(params![task, digest, task, path], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?,
        )
    }
}
impl Engine {
    pub(crate) fn record_rename_hint(&self, id: &str, hint: &watch::RenameHint) -> Result<()> {
        let t = self.index.task(id)?;
        if t.mode != Mode::TwoWay {
            return Ok(());
        }
        let root = match hint.side.as_str() {
            "A" => &t.a,
            "B" => &t.b,
            _ => anyhow::bail!("Invalid rename side"),
        };
        fs::safe_path(root, &hint.from)?;
        fs::safe_path(root, &hint.to)?;
        let matcher = fs::Matcher::new(&t.scope)?;
        if !matcher.allows(&hint.from, false) || !matcher.allows(&hint.to, false) {
            return Ok(());
        }
        let from = path_key(&hint.from);
        let to = path_key(&hint.to);
        if from == to {
            return Ok(());
        }
        let known:bool=self.index.db.query_row("SELECT EXISTS(SELECT 1 FROM baseline_identity WHERE task=? AND side=? AND path=? UNION ALL SELECT 1 FROM rename_links WHERE task=? AND target=?)",params![id,hint.side,from,id,from],|r|r.get(0))?;
        if known {
            self.index.link_candidate(id, &from, &to)?;
            self.queue_paths(id, &[hint.from.clone(), hint.to.clone()], false)?;
        }
        Ok(())
    }
    pub(crate) fn expand_lineage(
        &mut self,
        t: &Task,
        a: &mut Tree,
        b: &mut Tree,
        keys: &mut BTreeSet<String>,
        m: &mut Metrics,
    ) -> Result<()> {
        if t.mode != Mode::TwoWay {
            return Ok(());
        }
        let mut pending: VecDeque<_> = keys.iter().cloned().collect();
        while let Some(key) = pending.pop_front() {
            crate::progress::check_cancel(Some(&self.cancel))?;
            let mut related = self.index.linked(&t.id, &key)?;
            for (side, tree) in [("A", &*a), ("B", &*b)] {
                if let Some(id) = tree
                    .get(&key)
                    .filter(|e| !e.directory)
                    .and_then(|e| e.file_id.as_deref())
                {
                    related.extend(self.index.identity_origins(&t.id, side, id)?);
                }
                if let Some(entry) = tree.get(&key).filter(|e| !e.directory) {
                    related.extend(self.index.content_origins(&t.id, &key, &entry.digest)?);
                }
                // Previously indexed targets also remain reachable when the old
                // name's notification arrives after its target.
                let mut s=self.index.db.prepare("SELECT e.path FROM baseline_identity i JOIN entries e ON e.task=i.task AND e.side=i.side AND e.file_id=i.file_id WHERE i.task=? AND i.side=? AND i.path=?")?;
                related.extend(
                    s.query_map(params![t.id, side, key], |r| r.get::<_, String>(0))?
                        .collect::<rusqlite::Result<Vec<_>>>()?,
                );
            }
            for path in related {
                let path = path_key(&path);
                if !keys.insert(path.clone()) {
                    continue;
                }
                for (side, root, tree) in [("A", &t.a, &mut *a), ("B", &t.b, &mut *b)] {
                    let display = self
                        .index
                        .entry(&t.id, side, &path)?
                        .map(|e| e.path)
                        .unwrap_or_else(|| path.clone());
                    let entry = self.read_entry(root, &display, &t.scope, m)?;
                    self.index.put_entry(&t.id, side, &path, entry.as_ref())?;
                    if let Some(entry) = entry {
                        tree.insert(path.clone(), entry);
                    }
                }
                pending.push_back(path);
            }
        }
        Ok(())
    }
    pub(crate) fn protect_lineage(
        &self,
        t: &Task,
        operations: &mut [Operation],
        baselines: &BTreeMap<String, String>,
        metrics: &mut Metrics,
    ) -> Result<()> {
        if t.mode != Mode::TwoWay {
            return Ok(());
        }
        let positions: BTreeMap<_, _> = operations
            .iter()
            .enumerate()
            .map(|(i, o)| (path_key(&o.path), i))
            .collect();
        for operation in operations.iter() {
            let target = path_key(&operation.path);
            for (side, entry) in [("A", &operation.a), ("B", &operation.b)] {
                let Some(entry) = entry.as_ref().filter(|e| !e.directory) else {
                    continue;
                };
                let Some(id) = entry.file_id.as_deref() else {
                    continue;
                };
                for origin in self.index.identity_origins(&t.id, side, id)? {
                    if origin == target {
                        continue;
                    }
                    let Some(old) = positions.get(&origin).map(|i| &operations[*i]) else {
                        continue;
                    };
                    let old_side = if side == "A" { &old.a } else { &old.b };
                    if old_side.as_ref().and_then(|e| e.file_id.as_deref()) == Some(id) {
                        continue;
                    }
                    self.index.link_candidate(&t.id, &origin, &target)?;
                }
                for origin in self.index.content_origins(&t.id, &target, &entry.digest)? {
                    if origin == target {
                        continue;
                    }
                    let Some(old) = positions.get(&origin).map(|i| &operations[*i]) else {
                        continue;
                    };
                    if old.a.is_none() || old.b.is_none() {
                        self.index.link_candidate(&t.id, &origin, &target)?;
                    }
                }
            }
        }
        let mut s = self.index.db.prepare(
            "SELECT origin,target FROM rename_links WHERE task=? AND (origin=? OR target=?)",
        )?;
        let mut links = BTreeSet::new();
        for path in positions.keys() {
            let rows = s.query_map(params![t.id, path, path], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            for row in rows {
                metrics.lineage_links += 1;
                links.insert(row?);
            }
        }
        let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut origins = BTreeSet::new();
        let mut targets = BTreeSet::new();
        for (origin, target) in links {
            if positions.contains_key(&origin) && positions.contains_key(&target) {
                let known: bool = self.index.db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM baseline_identity WHERE task=? AND path=?)",
                    params![t.id, origin],
                    |r| r.get(0),
                )?;
                if known || baselines.contains_key(&origin) {
                    origins.insert(origin.clone());
                }
                targets.insert(target.clone());
                graph
                    .entry(origin.clone())
                    .or_default()
                    .insert(target.clone());
                graph.entry(target).or_default().insert(origin);
            }
        }
        let mut seen = BTreeSet::new();
        for first in graph.keys() {
            if seen.contains(first) {
                continue;
            }
            let mut stack = vec![first.clone()];
            let mut group = BTreeSet::new();
            while let Some(path) = stack.pop() {
                if !seen.insert(path.clone()) {
                    continue;
                }
                group.insert(path.clone());
                stack.extend(graph[&path].iter().cloned());
            }
            let old: Vec<_> = group.intersection(&origins).collect();
            let new: Vec<_> = group
                .intersection(&targets)
                .filter(|p| {
                    !origins.contains(*p) && {
                        let o = &operations[positions[*p]];
                        o.a.is_some() || o.b.is_some()
                    }
                })
                .collect();
            if new.is_empty() {
                continue;
            }
            let equal = |path: &str| {
                let o = &operations[positions[path]];
                o.a.as_ref().map(|e| (&e.digest, e.directory))
                    == o.b.as_ref().map(|e| (&e.digest, e.directory))
            };
            let resolved = old.iter().all(|p| equal(p))
                && new
                    .iter()
                    .all(|p| equal(p) && operations[positions[*p]].a.is_some());
            let simple = if old.len() == 1 && new.len() == 1 {
                let o = &operations[positions[old[0]]];
                let n = &operations[positions[new[0]]];
                let baseline = baselines.get(old[0]);
                match (&o.a, &o.b, &n.a, &n.b) {
                    (None, Some(b), Some(_), None) => baseline == Some(&b.digest),
                    (Some(a), None, None, Some(_)) => baseline == Some(&a.digest),
                    _ => false,
                }
            } else {
                false
            };
            if resolved || simple {
                continue;
            }
            for path in group {
                let current = &operations[positions[&path]];
                if current.a.is_none() && current.b.is_none() && !origins.contains(&path) {
                    continue;
                }
                let o = &mut operations[positions[&path]];
                o.action = Action::Conflict;
                o.reason = LINEAGE_RENAME.into();
            }
        }
        Ok(())
    }
    pub(crate) fn retire_lineage(&self, p: &Plan, manual: bool) -> Result<()> {
        let transaction = self.index.db.unchecked_transaction()?;
        for o in &p.operations {
            if matches!(o.action, Action::Conflict | Action::Ignore) {
                continue;
            }
            let path = path_key(&o.path);
            if !manual {
                if !matches!(o.action, Action::DeleteA | Action::DeleteB) || o.status != "Applied" {
                    continue;
                }
                let checkpoint:bool=self.index.db.query_row("SELECT EXISTS(SELECT 1 FROM rename_links l JOIN baseline b ON b.task=l.task AND b.path=l.target JOIN entries a ON a.task=b.task AND a.path=b.path AND a.side='A' AND a.digest=b.digest JOIN entries z ON z.task=b.task AND z.path=b.path AND z.side='B' AND z.digest=b.digest WHERE l.task=? AND l.origin=? AND b.digest IS NOT NULL)",params![p.task.id,path],|r|r.get(0))?;
                if !checkpoint {
                    continue;
                }
            }
            let absent: bool = self.index.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM baseline WHERE task=? AND path=? AND digest IS NULL)",
                params![p.task.id, path],
                |r| r.get(0),
            )?;
            if absent {
                self.index.db.execute(
                    "DELETE FROM baseline_identity WHERE task=? AND path=?",
                    params![p.task.id, path],
                )?;
                self.index.db.execute(
                    "DELETE FROM rename_links WHERE task=? AND (origin=? OR target=?)",
                    params![p.task.id, path, path],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }
}
