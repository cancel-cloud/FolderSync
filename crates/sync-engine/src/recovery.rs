use crate::*;
use anyhow::{Context, Result, ensure};
use rusqlite::params;
impl Engine {
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
            let Ok(t) = self.index.task(&id) else {
                continue;
            };
            if self.validate(&t).is_err() {
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
            let Ok(t) = self.index.task(&id) else {
                continue;
            };
            if self.validate(&t).is_err() {
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
        let json: String =
            self.index
                .db
                .query_row("SELECT json FROM versions WHERE id=?", [id], |r| r.get(0))?;
        let version: Version = serde_json::from_str(&json)?;
        let t = self.index.task(&version.task_id)?;
        self.validate(&t)?;
        let root = if version.side == "A" { &t.a } else { &t.b };
        let current = crate::fs::read_entry(
            root,
            &version.path,
            &Scope::default(),
            &mut Metrics::default(),
        )?;
        let p = RestorePreview {
            id: uuid::Uuid::new_v4().to_string(),
            generation: self.index.generation(&t.id)?,
            root_identity: crate::fs::root_identity(root)?,
            version,
            current,
        };
        self.set_setting(&format!("restore/{}", p.id), &serde_json::to_string(&p)?)?;
        Ok(p)
    }
    pub fn restore_apply(&mut self, id: &str) -> Result<()> {
        let json = self
            .setting(&format!("restore/{id}"))?
            .context("Restore preview missing or already used")?;
        let p: RestorePreview = serde_json::from_str(&json)?;
        let t = self.index.task(&p.version.task_id)?;
        self.validate(&t)?;
        ensure!(
            self.index.generation(&t.id)? == p.generation,
            "Task changed; review restore again"
        );
        let root = if p.version.side == "A" { &t.a } else { &t.b };
        ensure!(
            crate::fs::root_identity(root)? == p.root_identity,
            "Root changed"
        );
        let mut metrics = Metrics::default();
        ensure!(
            crate::fs::read_entry(root, &p.version.path, &Scope::default(), &mut metrics)?
                == p.current,
            "Restore destination changed; review again"
        );
        ensure!(
            p.version.stored.parent() == Some(self.storage().join("versions").as_path()),
            "Invalid protected version path"
        );
        ensure!(
            crate::fs::hash_file(&p.version.stored, &mut metrics)?.0 == p.version.digest,
            "Protected version failed integrity check"
        );
        if let Some(current) = &p.current {
            ensure!(!current.directory, "Cannot replace a folder with a file");
            self.backup(&t, &p.version.path, &p.version.side, current, &mut metrics)?;
        }
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
            "UPDATE tasks SET generation=generation+1 WHERE id=?",
            params![t.id],
        )?;
        self.cleanup_versions(&t)?;
        Ok(())
    }
}
