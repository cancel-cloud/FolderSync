use crate::{
    fs::{self as syncfs, path_key, validate_roots},
    index::Index,
    *,
};
use anyhow::{Context, Result, bail, ensure};
use rusqlite::{OptionalExtension, params};
use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub struct Engine {
    pub(crate) index: Index,
    pub cancel: Arc<AtomicBool>,
    pub progress: ProgressState,
}
impl Engine {
    pub fn preview_paths(&mut self, id: &str, paths: &[String]) -> Result<Plan> {
        let t = self.index.task(id)?;
        if t.comparison == Comparison::FullContent {
            return self.preview(id);
        }
        let queued_paths = paths.len() as u64;
        let mut paths = paths.to_vec();
        if t.conflict_policy == ConflictPolicy::RequireChoices {
            paths.extend(self.index.conflict_paths(id)?);
            paths.sort();
            paths.dedup();
        }
        self.cancel.store(false, Ordering::Relaxed);
        let _progress = self.progress.begin(id, None, ProgressPhase::Discovering, 0);
        let mut m = Metrics {
            queued_paths,
            ..Metrics::default()
        };
        let result = (|| {
            self.validate(&t)?;
            let mut keys = BTreeSet::new();
            let mut display_paths = std::collections::BTreeMap::new();
            let (mut a, mut b) = (Tree::new(), Tree::new());
            for path in &paths {
                let key = path_key(path);
                display_paths.insert(key.clone(), path.clone());
                keys.insert(key.clone());
                for (side, root) in [("A", &t.a), ("B", &t.b)] {
                    let existing = self.index.entry(id, side, &key)?;
                    if existing.as_ref().is_some_and(|e| e.directory) {
                        keys.extend(self.index.descendants(id, side, &key)?);
                    }
                    let actual = std::fs::symlink_metadata(syncfs::safe_path(root, path)?).ok();
                    if actual.as_ref().is_some_and(|e| e.is_dir()) {
                        let matcher = syncfs::Matcher::new(&t.scope)?;
                        let mut walk = walkdir::WalkDir::new(syncfs::safe_path(root, path)?)
                            .follow_links(false)
                            .min_depth(1)
                            .into_iter();
                        while let Some(item) = walk.next() {
                            crate::progress::check_cancel(Some(&self.cancel))?;
                            let item = item?;
                            let relative = item
                                .path()
                                .strip_prefix(root)?
                                .to_str()
                                .context("Non-Unicode path unsupported")?
                                .replace('\\', "/");
                            let directory = item.file_type().is_dir();
                            if !matcher.allows(&relative, directory)
                                || (directory
                                    && self
                                        .read_entry(root, &relative, &t.scope, &mut m)?
                                        .is_none())
                            {
                                if directory {
                                    walk.skip_current_dir()
                                }
                                continue;
                            }
                            let key = path_key(&relative);
                            keys.insert(key.clone());
                            display_paths.insert(key, relative);
                        }
                    }
                }
            }
            for key in &keys {
                for (side, root, tree) in [("A", &t.a, &mut a), ("B", &t.b, &mut b)] {
                    let display = self
                        .index
                        .entry(id, side, key)?
                        .map(|e| e.path)
                        .unwrap_or_else(|| {
                            display_paths
                                .get(key)
                                .cloned()
                                .unwrap_or_else(|| key.clone())
                        });
                    let entry = self.read_entry(root, &display, &t.scope, &mut m)?;
                    if let Some(e) = entry {
                        tree.insert(key.clone(), e);
                    }
                }
            }
            // A burst is one observation batch, not a durable disk flush for
            // every path/side. Failed reads leave the previous index intact.
            {
                let transaction = self.index.db.unchecked_transaction()?;
                for key in &keys {
                    for (side, tree) in [("A", &a), ("B", &b)] {
                        self.index.put_entry(id, side, key, tree.get(key))?;
                    }
                }
                transaction.commit()?;
            }
            self.expand_lineage(&t, &mut a, &mut b, &mut keys, &mut m)?;
            self.index.update_ancestors(id, "A", &keys)?;
            self.index.update_ancestors(id, "B", &keys)?;
            self.make_plan(t.clone(), &a, &b, keys, m.clone())
        })();
        if let Err(error) = &result {
            self.record_discovery_error(&t, &m, error)?;
        }
        result
    }
    pub fn open(storage: &Path) -> Result<Self> {
        let engine = Self {
            index: Index::open(storage)?,
            cancel: Arc::new(AtomicBool::new(false)),
            progress: ProgressState::default(),
        };
        engine.recover_legacy_catalog()?;
        engine.recover_displaced()?;
        engine.recover_stages()?;
        Ok(engine)
    }
    pub fn storage(&self) -> &Path {
        &self.index.storage
    }
    pub fn save_task(&mut self, t: &Task) -> Result<()> {
        ensure!(!t.name.trim().is_empty(), "Name is required");
        ensure!(uuid::Uuid::parse_str(&t.id).is_ok(), "Invalid task ID");
        ensure!(
            t.identifier == t.identifier.trim()
                && t.identifier.chars().count() <= 64
                && !t.identifier.chars().any(char::is_control),
            "Task identifier must be at most 64 characters without surrounding whitespace or control characters"
        );
        let identifier = if t.identifier.is_empty() {
            &t.id
        } else {
            &t.identifier
        }
        .to_lowercase();
        ensure!(
            !self.tasks()?.iter().any(|other| other.id != t.id
                && (if other.identifier.is_empty() {
                    &other.id
                } else {
                    &other.identifier
                })
                .to_lowercase()
                    == identifier),
            "Task identifier is already in use"
        );
        ensure!(
            !self.index.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM recovery_tasks WHERE id=?)",
                [&t.id],
                |r| r.get::<_, bool>(0)
            )?,
            "This task ID is reserved by deleted-task recovery; create a new task"
        );
        ensure!(
            t.debounce_secs <= 3600
                && (1..=16).contains(&t.transfer_concurrency)
                && t.bandwidth_kib.checked_mul(1024).is_some()
                && t.retries <= 10
                && t.history_cap > 0
                && t.history_cap <= 1000,
            "Invalid advanced settings"
        );
        syncfs::Matcher::new(&t.scope)?;
        crate::scheduler::next_due(&t.schedule, chrono::Local::now())?;
        let (a, b) = validate_roots(t, self.storage())?;
        self.validate_deletion_policy(t)?;
        if let Ok(old) = self.index.task(&t.id) {
            if old.schedule != t.schedule {
                self.index.db.execute(
                    "DELETE FROM settings WHERE key=?",
                    [format!("next/{}", t.id)],
                )?;
            }
            ensure!(
                old.a == t.a && old.b == t.b && old.mode == t.mode,
                "Changing roots or mode requires a new task to preserve baseline semantics"
            );
        }
        for other in self.tasks()?.into_iter().filter(|o| o.id != t.id) {
            if t.monitor
                || other.monitor
                || t.login
                || other.login
                || !t.schedule.kind.is_empty()
                || !other.schedule.kind.is_empty()
            {
                for x in [&t.a, &t.b] {
                    for y in [&other.a, &other.b] {
                        ensure!(
                            !syncfs::overlap(&fs::canonicalize(x)?, &fs::canonicalize(y)?),
                            "Automatic tasks cannot use overlapping roots"
                        );
                    }
                }
            }
        }
        self.index.db.execute("INSERT INTO tasks(id,json,root_a,root_b) VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET json=excluded.json,generation=generation+1",params![t.id,serde_json::to_string(t)?,a,b])?;
        Ok(())
    }
    pub fn tasks(&self) -> Result<Vec<Task>> {
        self.index.tasks()
    }
    /// Configuration-only batch: validate every identity before writing any row.
    pub fn assign_group(&mut self, ids: &[String], group: &str) -> Result<()> {
        let group = group.trim();
        ensure!(
            group.chars().count() <= 128 && !group.chars().any(char::is_control),
            "Invalid group name"
        );
        ensure!(!ids.is_empty(), "Select at least one task");
        let tasks = ids
            .iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|id| {
                let mut task = self.index.task(id)?;
                task.group = group.to_owned();
                Ok(task)
            })
            .collect::<Result<Vec<_>>>()?;
        let transaction = self.index.db.transaction()?;
        for task in tasks {
            transaction.execute(
                "UPDATE tasks SET json=?,generation=generation+1 WHERE id=?",
                params![serde_json::to_string(&task)?, task.id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
    pub fn statuses(&self) -> Result<Vec<TaskStatus>> {
        self.tasks()?.into_iter().map(|t|{
            let last:Option<String>=self.index.db.query_row("SELECT id FROM runs WHERE task=? AND status!='Preview' ORDER BY rowid DESC LIMIT 1",[&t.id],|r|r.get(0)).optional()?;
            let last_run=last.map(|id|self.plan_header(&id)).transpose()?;
            let pending_conflicts:i64=self.index.db.query_row("SELECT COUNT(*) FROM conflicts WHERE task=?",[&t.id],|r|r.get(0))?;
            Ok(TaskStatus{task_id:t.id.clone(),last_run,pending_conflicts:pending_conflicts as u64,next_run:self.setting(&format!("next/{}",t.id))?.and_then(|s|s.parse().ok()),deletion_baseline_run:self.deletion_baseline_run(&t)?})
        }).collect()
    }
    pub(crate) fn validate(&self, t: &Task) -> Result<(String, String)> {
        let roots = validate_roots(t, self.storage())?;
        ensure!(
            roots == self.index.roots(&t.id)?,
            "Root identity changed; review and recreate this task"
        );
        Ok(roots)
    }
    pub fn preview(&mut self, id: &str) -> Result<Plan> {
        self.cancel.store(false, Ordering::Relaxed);
        let _progress = self.progress.begin(id, None, ProgressPhase::Discovering, 0);
        let t = self.index.task(id)?;
        let mut m = Metrics {
            reconciliations: 1,
            ..Metrics::default()
        };
        let result = (|| {
            self.validate(&t)?;
            let mut a = syncfs::scan_controlled(
                &t.a,
                &t.scope,
                &mut m,
                Some(&self.cancel),
                Some(&self.progress),
            )?;
            let mut b = syncfs::scan_controlled(
                &t.b,
                &t.scope,
                &mut m,
                Some(&self.cancel),
                Some(&self.progress),
            )?;
            self.index.put_tree(id, "A", &a)?;
            self.index.put_tree(id, "B", &b)?;
            let pending = self.index.conflict_paths(id)?;
            let mut keys = a
                .keys()
                .chain(b.keys())
                .chain(self.index.baseline(id)?.keys())
                .chain(pending.iter())
                .cloned()
                .collect();
            self.expand_lineage(&t, &mut a, &mut b, &mut keys, &mut m)?;
            self.make_plan(t.clone(), &a, &b, keys, m.clone())
        })();
        if let Err(error) = &result {
            self.record_discovery_error(&t, &m, error)?;
        }
        result
    }
    pub(crate) fn make_plan(
        &self,
        t: Task,
        a: &Tree,
        b: &Tree,
        keys: BTreeSet<String>,
        mut metrics: Metrics,
    ) -> Result<Plan> {
        let roots = self.validate(&t)?;
        let mut operations = Vec::new();
        let mut baselines = std::collections::BTreeMap::new();
        let matcher = syncfs::Matcher::new(&t.scope)?;
        for key in keys {
            let baseline: Option<Option<String>> = self
                .index
                .db
                .prepare_cached("SELECT digest FROM baseline WHERE task=? AND path=?")?
                .query_row(params![t.id, key], |r| r.get(0))
                .optional()?;
            let ae = a.get(&key);
            let be = b.get(&key);
            let path = ae.or(be).map(|e| e.path.clone()).unwrap_or(key.clone());
            if !matcher.allows(&path, ae.or(be).is_some_and(|e| e.directory)) {
                self.index.db.execute(
                    "DELETE FROM conflicts WHERE task=? AND path=?",
                    params![t.id, key],
                )?;
                continue;
            }
            // An excluded existing object is never evidence of absence.
            if syncfs::excluded_attributes(&t.a, &path, &t.scope)?
                || syncfs::excluded_attributes(&t.b, &path, &t.scope)?
            {
                self.index.db.execute(
                    "DELETE FROM conflicts WHERE task=? AND path=?",
                    params![t.id, key],
                )?;
                continue;
            }
            if let Some(Some(digest)) = &baseline {
                baselines.insert(key.clone(), digest.clone());
            }
            let mut action = classify(
                t.mode,
                ae.map(|e| e.digest.as_str()),
                be.map(|e| e.digest.as_str()),
                baseline.as_ref().and_then(|x| x.as_deref()),
            );
            if ae.zip(be).is_some_and(|(x, y)| x.directory != y.directory) {
                action = Action::Conflict;
            }
            if action == Action::Conflict {
                self.index.db.execute(
                    "INSERT OR REPLACE INTO conflicts VALUES(?,?,?)",
                    params![t.id, key, serde_json::to_string(&(ae, be))?],
                )?;
            }
            operations.push(Operation {
                path,
                action,
                status: "Planned".into(),
                reason: match action {
                    Action::Conflict => {
                        "Both sides differ from a common baseline, or there is no safe baseline"
                    }
                    Action::DeleteA | Action::DeleteB => {
                        "Reviewed deletion; a protected version will be kept"
                    }
                    Action::MoveToB => "Verify destination before removing source",
                    Action::Unchanged => "No transfer required",
                    _ => "One-sided creation or change",
                }
                .into(),
                a: ae.cloned(),
                b: be.cloned(),
            });
        }
        crate::renames::protect_ambiguous(t.mode, &mut operations, &baselines);
        self.protect_lineage(&t, &mut operations, &baselines, &mut metrics)?;
        for operation in &operations {
            if operation.reason == crate::renames::AMBIGUOUS_RENAME
                || operation.reason == crate::lineage::LINEAGE_RENAME
            {
                self.index.db.execute(
                    "INSERT OR REPLACE INTO conflicts VALUES(?,?,?)",
                    params![
                        t.id,
                        path_key(&operation.path),
                        serde_json::to_string(operation)?
                    ],
                )?;
            }
        }
        let changed: Vec<_> = operations
            .iter()
            .filter(|o| {
                !matches!(
                    o.action,
                    Action::DeleteA | Action::DeleteB | Action::Unchanged
                )
            })
            .map(|o| path_key(&o.path))
            .collect();
        for operation in &mut operations {
            if matches!(operation.action, Action::DeleteA | Action::DeleteB)
                && operation
                    .a
                    .as_ref()
                    .or(operation.b.as_ref())
                    .is_some_and(|e| e.directory)
            {
                let prefix = format!("{}/", path_key(&operation.path));
                if changed.iter().any(|p| p.starts_with(&prefix)) {
                    operation.action = Action::Conflict;
                    operation.reason="Directory deletion conflicts with changed descendants; originals preserved".into();
                    self.index.db.execute(
                        "INSERT OR REPLACE INTO conflicts VALUES(?,?,?)",
                        params![
                            t.id,
                            path_key(&operation.path),
                            serde_json::to_string(operation)?
                        ],
                    )?;
                }
            }
        }
        let p = Plan {
            id: uuid::Uuid::new_v4().to_string(),
            baseline_generation: self.index.generation(&t.id)?,
            task: t,
            root_a: roots.0,
            root_b: roots.1,
            operations,
            metrics,
            created_at: now(),
            status: "Preview".into(),
            deletion_review: None,
        };
        self.index.put_plan(&p)?;
        Ok(p)
    }
    pub fn resolve(&mut self, id: &str, path: &str, choice: &str) -> Result<Plan> {
        let mut p = self.index.plan(id)?;
        ensure!(
            matches!(p.status.as_str(), "Preview" | "Review required"),
            "Only previews can be revised"
        );
        let o = p
            .operations
            .iter_mut()
            .find(|o| o.path == path && matches!(o.action, Action::Conflict | Action::Ignore))
            .context("Conflict not found")?;
        o.action = match choice {
            "a" if o.a.is_some() => Action::CopyToB,
            "b" if o.b.is_some() => Action::CopyToA,
            "ignore" => Action::Ignore,
            "keep" => Action::Conflict,
            _ => bail!("Choose an existing side or leave unresolved"),
        };
        o.reason = if o.action == Action::Ignore {
            "Explicit ignore for this run; conflict remains unresolved"
        } else if choice == "keep" {
            "Both originals preserved; conflict remains unresolved"
        } else {
            "Explicit conflict choice; Apply is still required"
        }
        .into();
        p.id = uuid::Uuid::new_v4().to_string();
        self.index.put_plan(&p)?;
        Ok(p)
    }
    pub fn apply(&mut self, id: &str, destructive: bool) -> Result<Plan> {
        self.apply_internal(id, destructive, true)
    }
    pub(crate) fn apply_internal(
        &mut self,
        id: &str,
        destructive: bool,
        manual: bool,
    ) -> Result<Plan> {
        self.cancel.store(false, Ordering::Relaxed);
        let mut p = self.index.plan(id)?;
        let _progress = self.progress.begin(
            &p.task.id,
            Some(id),
            ProgressPhase::Validating,
            p.operations.len(),
        );
        ensure!(
            matches!(p.status.as_str(), "Preview" | "Review required"),
            "Plan already applied or interrupted; create a new preview"
        );
        ensure!(
            p.task.conflict_policy != ConflictPolicy::RequireChoices
                || !p.operations.iter().any(|o| o.action == Action::Conflict),
            "Choose a copy direction or Ignore for every conflict before applying this task"
        );
        ensure!(
            self.index.task(&p.task.id)? == p.task
                && self.index.generation(&p.task.id)? == p.baseline_generation,
            "Task or baseline changed; preview again"
        );
        if p.task.conflict_policy == ConflictPolicy::RequireChoices {
            let reviewed: BTreeSet<_> = p.operations.iter().map(|o| path_key(&o.path)).collect();
            ensure!(
                self.index
                    .conflict_paths(&p.task.id)?
                    .iter()
                    .all(|path| reviewed.contains(path)),
                "Task has newly discovered conflicts; preview again"
            );
        }
        let roots = self.validate(&p.task)?;
        ensure!(
            roots == (p.root_a.clone(), p.root_b.clone()),
            "Roots changed"
        );
        ensure!(
            destructive
                || !p.operations.iter().any(|o| matches!(
                    o.action,
                    Action::DeleteA | Action::DeleteB | Action::MoveToB
                ) || o.reason.starts_with("Explicit")),
            "Destructive review is required"
        );
        // Fresh automatic equal observations do not mutate either root. Their
        // verified common content can advance the baseline without reading both
        // trees again. Later events (or restart recovery) compare against that
        // observation. Manual previews and every plan with a non-equal action
        // retain the complete stale-input barrier before any mutation.
        if manual || p.operations.iter().any(|o| o.action != Action::Unchanged) {
            self.validate_inputs(&p.task, &p.operations, &mut p.metrics)?;
        }
        // Persist all mutation intents before touching a file. One durable
        // transaction provides the same write-ahead boundary without a flush
        // for every small file. Pending does not mean that a file was changed.
        {
            let transaction = self.index.db.transaction()?;
            transaction.execute("UPDATE runs SET status='Applying' WHERE id=?", [id])?;
            {
                let mut insert = transaction.prepare(
                    "INSERT OR REPLACE INTO journal(run,path,json,status) VALUES(?,?,?,'Pending')",
                )?;
                for operation in &p.operations {
                    if !matches!(
                        operation.action,
                        Action::Conflict | Action::Ignore | Action::Unchanged
                    ) {
                        insert.execute(params![
                            id,
                            operation.path,
                            serde_json::to_string(operation)?
                        ])?;
                    }
                }
            }
            transaction.commit()?;
        }
        // Equal snapshots need no mutation journal. Commit their baseline in one
        // transaction instead of forcing a disk flush for every unchanged file.
        {
            let tx = self.index.db.transaction()?;
            for o in p
                .operations
                .iter()
                .filter(|o| o.action == Action::Unchanged)
            {
                if o.a.as_ref().map(|e| &e.digest) == o.b.as_ref().map(|e| &e.digest) {
                    tx.execute(
                        "INSERT OR REPLACE INTO baseline VALUES(?,?,?)",
                        params![
                            p.task.id,
                            path_key(&o.path),
                            o.a.as_ref().map(|e| &e.digest)
                        ],
                    )?;
                    tx.execute(
                        "DELETE FROM conflicts WHERE task=? AND path=?",
                        params![p.task.id, path_key(&o.path)],
                    )?;
                    Index::remember_common(
                        &tx,
                        &p.task.id,
                        &path_key(&o.path),
                        o.a.as_ref(),
                        o.b.as_ref(),
                    )?;
                }
            }
            tx.commit()?;
        }
        p.operations.sort_by_key(|o| {
            let deleting = matches!(o.action, Action::DeleteA | Action::DeleteB);
            let dir = o.a.as_ref().or(o.b.as_ref()).is_some_and(|e| e.directory);
            (
                if deleting {
                    2
                } else if dir {
                    0
                } else {
                    1
                },
                if deleting {
                    usize::MAX - o.path.matches('/').count()
                } else {
                    o.path.matches('/').count()
                },
            )
        });
        for operation in &mut p.operations {
            operation.status = "Not run".into();
        }
        let result = (|| -> Result<()> {
            self.execute_operations(id, &p.task, &mut p.operations, &mut p.metrics)?;
            let mut dirs: Vec<_> = p
                .operations
                .iter()
                .filter(|o| {
                    o.action == Action::MoveToB && o.a.as_ref().is_some_and(|e| e.directory)
                })
                .collect();
            dirs.sort_by_key(|o| std::cmp::Reverse(o.path.matches('/').count()));
            for o in dirs {
                let path = syncfs::safe_path(&p.task.a, &o.path)?;
                fs::remove_dir(path)?;
                self.index
                    .put_entry(&p.task.id, "A", &path_key(&o.path), None)?;
            }
            Ok(())
        })();
        if let Err(error) = &result {
            for operation in &mut p.operations {
                if operation.status == "Applying" {
                    operation.status = if error.is::<Cancelled>() {
                        "Cancelled".into()
                    } else {
                        "Failed".into()
                    };
                }
            }
        }
        let cleanup_error = if result.is_ok() {
            self.cleanup_versions(&p.task).err()
        } else {
            None
        };
        p.status = match &result {
            Ok(_) => {
                if p.operations
                    .iter()
                    .any(|o| matches!(o.action, Action::Conflict | Action::Ignore))
                {
                    "Attention".into()
                } else {
                    "Applied".into()
                }
            }
            Err(e) if e.is::<crate::progress::Cancelled>() => "Cancelled".into(),
            Err(e) => format!("Failed: {e:#}"),
        };
        if let Some(error) = cleanup_error {
            p.status
                .push_str(&format!("; retention cleanup pending: {error}"));
        }
        if result.is_ok() {
            self.retire_lineage(&p, manual)?;
        }
        let transaction = self.index.db.transaction()?;
        transaction.execute(
            "UPDATE tasks SET generation=generation+1 WHERE id=?",
            [&p.task.id],
        )?;
        transaction.execute(
            "UPDATE runs SET json=?,status=? WHERE id=?",
            params![serde_json::to_string(&p)?, p.status, p.id],
        )?;
        if manual && result.is_ok() && p.status == "Applied" && p.metrics.reconciliations == 1 {
            transaction.execute(
                "INSERT OR REPLACE INTO baseline_reviews VALUES(?,?)",
                params![p.task.id, crate::deletion::manual_review_json(&p)?],
            )?;
        }
        transaction.commit()?;
        let keys = p.operations.iter().map(|o| path_key(&o.path)).collect();
        self.index.update_ancestors(&p.task.id, "A", &keys)?;
        self.index.update_ancestors(&p.task.id, "B", &keys)?;
        self.write_log(&p)?;
        result?;
        Ok(p)
    }
    pub(crate) fn backup(
        &self,
        t: &Task,
        path: &str,
        side: &str,
        entry: &Entry,
        m: &mut Metrics,
    ) -> Result<()> {
        if entry.directory || !t.history_enabled {
            return Ok(());
        }
        let root = if side == "A" { &t.a } else { &t.b };
        let source = syncfs::safe_path(root, path)?;
        let id = uuid::Uuid::new_v4().to_string();
        let stored = self.new_version_path(&t.id, &id)?;
        self.transfer(&source, &stored, &entry.digest, None, t, m)?;
        let v = Version {
            id,
            task_id: t.id.clone(),
            side: side.into(),
            path: path.into(),
            digest: entry.digest.clone(),
            stored,
            created_at: now(),
        };
        self.index.db.execute(
            "INSERT INTO versions VALUES(?,?,?)",
            params![v.id, t.id, serde_json::to_string(&v)?],
        )?;
        Ok(())
    }
    fn copy(
        &self,
        t: &Task,
        path: &str,
        to_b: bool,
        expected: &Entry,
        dest: Option<&Entry>,
        m: &mut Metrics,
    ) -> Result<()> {
        let source_root = if to_b { &t.a } else { &t.b };
        let target_root = if to_b { &t.b } else { &t.a };
        ensure!(
            self.read_entry(source_root, path, &t.scope, m)?.as_ref() == Some(expected),
            "Source changed before transfer"
        );
        ensure!(
            self.read_entry(target_root, path, &t.scope, m)?.as_ref() == dest,
            "Destination changed before transfer"
        );
        let source = syncfs::safe_path(source_root, path)?;
        let target = syncfs::safe_path(target_root, path)?;
        if expected.directory {
            fs::create_dir_all(target)?;
            return Ok(());
        }
        if let Some(old) = dest {
            if old.digest == expected.digest {
                return Ok(());
            }
            self.backup(t, path, if to_b { "B" } else { "A" }, old, m)?;
        }
        fs::create_dir_all(target.parent().context("Missing parent")?)?;
        self.transfer(
            &source,
            &target,
            &expected.digest,
            dest.map(|e| e.digest.as_str()),
            t,
            m,
        )?;
        Ok(())
    }
    fn remove(
        &self,
        t: &Task,
        path: &str,
        side: &str,
        expected: &Entry,
        require_other_absent: bool,
        m: &mut Metrics,
    ) -> Result<()> {
        let root = if side == "A" { &t.a } else { &t.b };
        let check_absence = || -> Result<()> {
            if !require_other_absent {
                return Ok(());
            }
            self.validate(t)?;
            let other = if side == "A" { &t.b } else { &t.a };
            match fs::symlink_metadata(syncfs::safe_path(other, path)?) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.into()),
                Ok(_) => bail!("Deleted side reappeared; originals preserved; preview again"),
            }
        };
        check_absence()?;
        let actual = self
            .read_entry(root, path, &t.scope, m)?
            .context("Delete input vanished")?;
        ensure!(
            actual.digest == expected.digest && actual.directory == expected.directory,
            "Delete input changed"
        );
        let target = syncfs::safe_path(root, path)?;
        if expected.directory {
            fs::remove_dir(target)?;
        } else {
            #[cfg(windows)]
            let delete_guard = {
                use std::os::windows::fs::OpenOptionsExt;
                fs::OpenOptions::new()
                    .access_mode(0x80010000)
                    .share_mode(1)
                    .open(&target)?
            };
            ensure!(
                self.read_entry(root, path, &Scope::default(), m)?.as_ref() == Some(expected),
                "Delete input changed before guard"
            );
            self.backup(t, path, side, expected, m)?;
            ensure!(
                self.read_entry(root, path, &t.scope, m)?.as_ref() == Some(expected),
                "Delete input changed during backup"
            );
            check_absence()?;
            #[cfg(windows)]
            {
                use std::os::windows::io::AsRawHandle;
                use windows_sys::Win32::Storage::FileSystem::{
                    FILE_DISPOSITION_INFO, FileDispositionInfo, SetFileInformationByHandle,
                };
                let info = FILE_DISPOSITION_INFO { DeleteFile: true };
                ensure!(
                    unsafe {
                        SetFileInformationByHandle(
                            delete_guard.as_raw_handle(),
                            FileDispositionInfo,
                            (&info as *const FILE_DISPOSITION_INFO).cast(),
                            std::mem::size_of_val(&info) as u32,
                        )
                    } != 0,
                    "Protected deletion failed: {}",
                    std::io::Error::last_os_error()
                );
            }
            #[cfg(not(windows))]
            fs::remove_file(target)?;
        }
        Ok(())
    }
    pub fn history(&self, task: &str) -> Result<Vec<Plan>> {
        self.index.history(task)
    }
    pub fn versions(&self, task: &str) -> Result<Vec<Version>> {
        self.index.versions(task)
    }
    pub fn delete_task(&mut self, id: &str) -> Result<()> {
        let pending: i64 =
            self.index
                .db
                .query_row("SELECT COUNT(*) FROM displaced WHERE task=?", [id], |r| {
                    r.get(0)
                })?;
        ensure!(
            pending == 0,
            "Protected recovery is pending; reconnect roots and restart before deleting this task"
        );
        let mut task = self.index.task(id)?;
        task.monitor = false;
        task.login = false;
        task.schedule = Schedule::default();
        task.deletion_policy.enabled = false;
        let (a, b) = self.index.roots(id)?;
        let generation = self.index.generation(id)? + 1;
        let transaction = self.index.db.transaction()?;
        transaction.execute(
            "INSERT INTO recovery_tasks VALUES(?,?,?,?,?,?)",
            params![
                id,
                serde_json::to_string(&task)?,
                a,
                b,
                generation,
                i64::try_from(now())?
            ],
        )?;
        transaction.execute("DELETE FROM dirty WHERE task=?", [id])?;
        transaction.execute("DELETE FROM merkle WHERE task=?", [id])?;
        for prefix in ["next/", "watch-events/", "notification/"] {
            transaction.execute(
                "DELETE FROM settings WHERE key=?",
                [format!("{prefix}{id}")],
            )?;
        }
        transaction.execute("DELETE FROM tasks WHERE id=?", [id])?;
        transaction.commit()?;
        Ok(())
    }
    pub(crate) fn apply_operation(
        &mut self,
        id: &str,
        task: &Task,
        o: &mut Operation,
        metrics: &mut Metrics,
        completed: usize,
    ) -> Result<()> {
        self.progress.update(|s| {
            s.phase = ProgressPhase::Applying;
            s.completed_entries = completed as u64;
            s.path = Some(o.path.clone());
            s.file_bytes = 0;
            s.file_total = 0;
        });
        if matches!(
            o.action,
            Action::Conflict | Action::Ignore | Action::Unchanged
        ) {
            o.status = if o.action == Action::Ignore {
                "Ignored".into()
            } else if o.action == Action::Conflict {
                "Conflict".into()
            } else {
                "Unchanged".into()
            };
            return Ok(());
        }
        o.status = "Applying".into();
        metrics.peak_workers = metrics.peak_workers.max(1);
        crate::progress::check_cancel(Some(&self.cancel))?;
        self.validate(task)?;
        match o.action {
            Action::CopyToB | Action::MoveToB => self.copy(
                task,
                &o.path,
                true,
                o.a.as_ref().context("Source missing")?,
                o.b.as_ref(),
                metrics,
            )?,
            Action::CopyToA => self.copy(
                task,
                &o.path,
                false,
                o.b.as_ref().context("Source missing")?,
                o.a.as_ref(),
                metrics,
            )?,
            Action::DeleteA => self.remove(
                task,
                &o.path,
                "A",
                o.a.as_ref().context("Missing delete input")?,
                true,
                metrics,
            )?,
            Action::DeleteB => self.remove(
                task,
                &o.path,
                "B",
                o.b.as_ref().context("Missing delete input")?,
                true,
                metrics,
            )?,
            _ => {}
        }
        if o.action == Action::MoveToB && !o.a.as_ref().is_some_and(|e| e.directory) {
            let destination = syncfs::safe_path(&task.b, &o.path)?;
            let _destination_guard = syncfs::read_guard(&destination)?;
            ensure!(
                syncfs::hash_file_controlled(
                    &destination,
                    metrics,
                    Some(&self.cancel),
                    Some(&self.progress)
                )?
                .0 == o.a.as_ref().unwrap().digest,
                "Move destination changed; source preserved"
            );
            self.remove(task, &o.path, "A", o.a.as_ref().unwrap(), false, metrics)?;
        }
        o.status = "Applied".into();
        let (a, b) = (
            self.read_entry(&task.a, &o.path, &task.scope, metrics)?,
            self.read_entry(&task.b, &o.path, &task.scope, metrics)?,
        );
        let digest = if task.mode == Mode::TwoWay {
            if a.as_ref().map(|e| &e.digest) != b.as_ref().map(|e| &e.digest) {
                self.index.db.execute(
                    "UPDATE journal SET status='Complete' WHERE run=? AND path=?",
                    params![id, o.path],
                )?;
                return Ok(());
            }
            a.as_ref().map(|e| e.digest.clone())
        } else if task.mode == Mode::Move {
            None
        } else {
            a.as_ref()
                .filter(|x| b.as_ref().is_some_and(|y| x.digest == y.digest))
                .map(|e| e.digest.clone())
        };
        let tx = self.index.db.unchecked_transaction()?;
        tx.execute(
            "UPDATE journal SET status='Complete' WHERE run=? AND path=?",
            params![id, o.path],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO baseline VALUES(?,?,?)",
            params![task.id, path_key(&o.path), digest],
        )?;
        tx.execute(
            "DELETE FROM conflicts WHERE task=? AND path=?",
            params![task.id, path_key(&o.path)],
        )?;
        Index::remember_common(&tx, &task.id, &path_key(&o.path), a.as_ref(), b.as_ref())?;
        self.index
            .put_entry(&task.id, "A", &path_key(&o.path), a.as_ref())?;
        self.index
            .put_entry(&task.id, "B", &path_key(&o.path), b.as_ref())?;
        tx.commit()?;
        Ok(())
    }
    pub fn export(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(
            &serde_json::json!({"schema":1,"tasks":self.tasks()?}),
        )?)
    }
    pub fn import(&mut self, json: &str) -> Result<usize> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        ensure!(value["schema"] == 1, "Unsupported schema");
        let mut tasks: Vec<Task> = serde_json::from_value(value["tasks"].clone())?;
        ensure!(tasks.len() <= 1000, "Too many tasks");
        let mut identifiers: BTreeSet<String> = self
            .tasks()?
            .iter()
            .map(|t| {
                (if t.identifier.is_empty() {
                    &t.id
                } else {
                    &t.identifier
                })
                .to_lowercase()
            })
            .collect();
        for t in &mut tasks {
            t.id = uuid::Uuid::new_v4().to_string();
            if !t.identifier.is_empty() && !identifiers.insert(t.identifier.to_lowercase()) {
                t.identifier.clear();
            }
            if t.identifier.is_empty() {
                identifiers.insert(t.id.to_lowercase());
            }
            t.monitor = false;
            t.login = false;
            t.schedule = Schedule::default();
            t.deletion_policy.enabled = false;
            validate_roots(t, self.storage())?;
            syncfs::Matcher::new(&t.scope)?;
        }
        self.index.db.execute_batch("SAVEPOINT import_batch")?;
        for t in &tasks {
            if let Err(error) = self.save_task(t) {
                self.index
                    .db
                    .execute_batch("ROLLBACK TO import_batch; RELEASE import_batch")?;
                return Err(error);
            }
        }
        self.index.db.execute_batch("RELEASE import_batch")?;
        Ok(tasks.len())
    }
    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .index
            .db
            .query_row("SELECT json FROM settings WHERE key=?", [key], |r| r.get(0))
            .optional()?)
    }
    pub fn set_setting(&self, key: &str, json: &str) -> Result<()> {
        self.index.db.execute(
            "INSERT OR REPLACE INTO settings VALUES(?,?)",
            params![key, json],
        )?;
        Ok(())
    }
}
