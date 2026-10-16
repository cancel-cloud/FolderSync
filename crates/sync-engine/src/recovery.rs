use crate::*;
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
impl Engine {
    pub fn recovery_tasks(&self) -> Result<Vec<RecoveryTask>> {
        let mut statement=self.index.db.prepare("SELECT r.json,r.archived_at,count(v.id) FROM recovery_tasks r JOIN versions v ON v.task=r.id GROUP BY r.id ORDER BY r.archived_at DESC,r.id")?;
        let rows = statement.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        rows.map(|r| {
            let (json, archived_at, versions) = r?;
            Ok(RecoveryTask {
                task: serde_json::from_str(&json)?,
                archived_at: u64::try_from(archived_at)?,
                versions: u64::try_from(versions)?,
            })
        })
        .collect()
    }
    pub(crate) fn recover_legacy_catalog(&self) -> Result<()> {
        let mut statement=self.index.db.prepare("SELECT DISTINCT v.task FROM versions v WHERE NOT EXISTS(SELECT 1 FROM tasks t WHERE t.id=v.task) AND NOT EXISTS(SELECT 1 FROM recovery_tasks r WHERE r.id=v.task)")?;
        let ids = statement
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        for id in ids {
            let json: Option<String> = self
                .index
                .db
                .query_row(
                    "SELECT json FROM runs WHERE task=? ORDER BY rowid DESC LIMIT 1",
                    [&id],
                    |r| r.get(0),
                )
                .optional()?;
            let Some(plan) = json.and_then(|j| serde_json::from_str::<Plan>(&j).ok()) else {
                continue;
            };
            if plan.task.id != id || plan.root_a.is_empty() || plan.root_b.is_empty() {
                continue;
            }
            let mut task = plan.task;
            task.monitor = false;
            task.login = false;
            task.schedule = Schedule::default();
            task.deletion_policy.enabled = false;
            self.index.db.execute(
                "INSERT INTO recovery_tasks VALUES(?,?,?,?,?,?)",
                params![
                    id,
                    serde_json::to_string(&task)?,
                    plan.root_a,
                    plan.root_b,
                    plan.baseline_generation.saturating_add(2),
                    i64::try_from(now())?
                ],
            )?;
        }
        Ok(())
    }
    fn validate_recovery_pair(&self, t: &Task) -> Result<()> {
        let (_, _, _, a, b) = self.index.recovery_context(&t.id)?;
        ensure!(
            crate::fs::validate_roots(t, self.storage())? == (a, b),
            "Recovery roots changed"
        );
        Ok(())
    }
    fn validate_recovery_root(&self, t: &Task, side: &str) -> Result<String> {
        ensure!(matches!(side, "A" | "B"), "Invalid recovery side");
        let (_, _, _, a, b) = self.index.recovery_context(&t.id)?;
        let root = if side == "A" { &t.a } else { &t.b };
        let identity = crate::fs::root_identity(root)?;
        ensure!(
            identity == if side == "A" { a } else { b },
            "Recovery root changed; original root identity is required"
        );
        Ok(identity)
    }
    pub(crate) fn preserve_displaced(
        &self,
        path: &std::path::Path,
        t: &Task,
        relative: &str,
        side: &str,
    ) -> Result<Option<String>> {
        let mut m = Metrics::default();
        let digest = match std::fs::symlink_metadata(path) {
            Ok(metadata) => {
                ensure!(
                    metadata.is_file() && !metadata.is_symlink(),
                    "Unsafe displaced object"
                );
                Some(crate::fs::hash_file(path, &mut m)?.0)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        if let Some(digest) = &digest {
            // The planned old version was normally protected before staging.
            // Reuse a verified copy rather than consuming the retention cap twice.
            let already_protected = self.versions(&t.id)?.iter().any(|v| {
                v.side == side
                    && crate::fs::path_key(&v.path) == crate::fs::path_key(relative)
                    && &v.digest == digest
                    && crate::fs::hash_file(&v.stored, &mut Metrics::default())
                        .is_ok_and(|(d, _)| d == *digest)
            });
            if already_protected {
                std::fs::remove_file(path)?;
                self.index.db.execute(
                    "DELETE FROM displaced WHERE path=?",
                    [path.to_string_lossy().as_ref()],
                )?;
                return Ok(Some(digest.clone()));
            }
            let id = uuid::Uuid::new_v4().to_string();
            let stored = self.storage().join("versions").join(&id);
            self.transfer(path, &stored, digest, None, t, &mut m)?;
            let v = Version {
                id,
                task_id: t.id.clone(),
                side: side.into(),
                path: relative.replace('\\', "/"),
                digest: digest.clone(),
                stored,
                created_at: now(),
            };
            self.index.db.execute(
                "INSERT INTO versions VALUES(?,?,?)",
                params![v.id, t.id, serde_json::to_string(&v)?],
            )?;
            std::fs::remove_file(path)?;
        }
        self.index.db.execute(
            "DELETE FROM displaced WHERE path=?",
            [path.to_string_lossy().as_ref()],
        )?;
        Ok(digest)
    }
    pub(crate) fn recover_displaced(&self) -> Result<()> {
        let mut statement = self
            .index
            .db
            .prepare("SELECT path,task,relative,side FROM displaced")?;
        let rows = statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        for (path, id, relative, side) in rows {
            let Ok((t, _, _, _, _)) = self.index.recovery_context(&id) else {
                continue;
            };
            if self.validate_recovery_pair(&t).is_err() {
                continue;
            }
            let root = if side == "A" { &t.a } else { &t.b };
            let path = std::path::PathBuf::from(path);
            ensure!(
                path.file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.starts_with(".foldersync-displaced-")),
                "Unsafe displaced path"
            );
            let rel = path.strip_prefix(std::fs::canonicalize(root)?)?;
            crate::fs::safe_path(root, &rel.to_string_lossy())?;
            self.preserve_displaced(&path, &t, &relative, &side)?;
        }
        Ok(())
    }
    pub(crate) fn cleanup_versions(&self, t: &Task) -> Result<()> {
        let matcher = crate::fs::Matcher::new(&Scope {
            include: t.history_patterns.clone(),
            ..Scope::default()
        })?;
        let mut counts = std::collections::BTreeMap::new();
        for version in self.versions(&t.id)? {
            let count = counts
                .entry((version.side.clone(), crate::fs::path_key(&version.path)))
                .or_insert(0u32);
            *count += 1;
            let cap = if t.history_patterns.is_empty() || matcher.allows(&version.path, false) {
                t.history_cap
            } else {
                1
            };
            if *count <= cap {
                continue;
            }
            ensure!(
                uuid::Uuid::parse_str(&version.id).is_ok(),
                "Invalid protected version identity"
            );
            let expected = self.storage().join("versions").join(&version.id);
            ensure!(
                version.stored == expected,
                "Protected version path mismatch"
            );
            let target = crate::fs::safe_path(self.storage(), &format!("versions/{}", version.id))?;
            // Remove the catalog reference first: interruption can leave an
            // orphaned protected file, never a dangling restore entry.
            self.index
                .db
                .execute("DELETE FROM versions WHERE id=?", [&version.id])?;
            match std::fs::remove_file(target) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
    pub(crate) fn recover_stages(&self) -> Result<()> {
        let mut s = self.index.db.prepare("SELECT path,task FROM staging")?;
        let rows = s
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(s);
        for (path, id) in rows {
            let Ok((t, _, _, _, _)) = self.index.recovery_context(&id) else {
                continue;
            };
            if self.validate_recovery_pair(&t).is_err() {
                continue;
            }
            let path = std::path::PathBuf::from(path);
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !name.starts_with(".foldersync-stage-") {
                continue;
            }
            let mut checked = None;
            for root in [&t.a, &t.b, self.storage()] {
                if let Ok(relative) = path.strip_prefix(root) {
                    checked = Some(crate::fs::safe_path(root, &relative.to_string_lossy())?);
                    break;
                }
            }
            let Some(checked) = checked else { continue };
            match std::fs::symlink_metadata(&checked) {
                Ok(m) => {
                    ensure!(m.is_file() && !m.is_symlink(), "Unsafe owned staging file");
                    std::fs::remove_file(&checked)?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
            self.index.db.execute(
                "DELETE FROM staging WHERE path=?",
                [path.to_string_lossy().as_ref()],
            )?;
        }
        Ok(())
    }
    pub fn restore_preview(&self, id: &str) -> Result<RestorePreview> {
        self.cancel
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let json: String =
            self.index
                .db
                .query_row("SELECT json FROM versions WHERE id=?", [id], |r| r.get(0))?;
        let version: Version = serde_json::from_str(&json)?;
        let (t, archived, generation, _, _) = self.index.recovery_context(&version.task_id)?;
        let _progress = self
            .progress
            .begin(&t.id, None, ProgressPhase::Validating, 1);
        let root_identity = self.validate_recovery_root(&t, &version.side)?;
        let root = if version.side == "A" { &t.a } else { &t.b };
        let current = self.read_entry(
            root,
            &version.path,
            &Scope::default(),
            &mut Metrics::default(),
        )?;
        let p = RestorePreview {
            id: uuid::Uuid::new_v4().to_string(),
            generation,
            root_identity,
            target_root: root.clone(),
            archived,
            version,
            current,
        };
        self.set_setting(&format!("restore/{}", p.id), &serde_json::to_string(&p)?)?;
        Ok(p)
    }
    pub fn restore_apply(&mut self, id: &str) -> Result<()> {
        self.cancel
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let json = self
            .setting(&format!("restore/{id}"))?
            .context("Restore preview missing or already used")?;
        let p: RestorePreview = serde_json::from_str(&json)?;
        let (t, archived, generation, _, _) = self.index.recovery_context(&p.version.task_id)?;
        let _progress = self
            .progress
            .begin(&t.id, None, ProgressPhase::Validating, 1);
        self.progress
            .update(|s| s.path = Some(p.version.path.clone()));
        self.validate_recovery_root(&t, &p.version.side)?;
        ensure!(
            generation == p.generation && archived == p.archived,
            "Task changed; review restore again"
        );
        let root = if p.version.side == "A" { &t.a } else { &t.b };
        ensure!(
            *root == p.target_root,
            "Restore target changed; preview again"
        );
        ensure!(
            crate::fs::root_identity(root)? == p.root_identity,
            "Root changed"
        );
        let mut metrics = Metrics::default();
        ensure!(
            self.read_entry(root, &p.version.path, &Scope::default(), &mut metrics)? == p.current,
            "Restore destination changed; review again"
        );
        ensure!(
            p.version.stored.parent() == Some(self.storage().join("versions").as_path()),
            "Invalid protected version path"
        );
        ensure!(
            crate::fs::hash_file_controlled(
                &p.version.stored,
                &mut metrics,
                Some(&self.cancel),
                Some(&self.progress)
            )?
            .0 == p.version.digest,
            "Protected version failed integrity check"
        );
        if let Some(current) = &p.current {
            ensure!(!current.directory, "Cannot replace a folder with a file");
            self.backup(&t, &p.version.path, &p.version.side, current, &mut metrics)?;
        }
        self.validate_recovery_root(&t, &p.version.side)?;
        let target = crate::fs::safe_path(root, &p.version.path)?;
        std::fs::create_dir_all(target.parent().context("Missing parent")?)?;
        self.transfer(
            &p.version.stored,
            &target,
            &p.version.digest,
            p.current.as_ref().map(|e| e.digest.as_str()),
            &t,
            &mut metrics,
        )?;
        self.index.db.execute(
            "DELETE FROM settings WHERE key=?",
            [format!("restore/{id}")],
        )?;
        self.index.db.execute(
            if archived {
                "UPDATE recovery_tasks SET generation=generation+1 WHERE id=?"
            } else {
                "UPDATE tasks SET generation=generation+1 WHERE id=?"
            },
            params![t.id],
        )?;
        self.cleanup_versions(&t)?;
        Ok(())
    }
}
