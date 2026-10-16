use crate::*;
use anyhow::{Context, Result, ensure};
use rusqlite::params;
impl Engine {
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
        let current =
            crate::fs::read_entry(root, &version.path, &t.scope, &mut Metrics::default())?;
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
            crate::fs::read_entry(root, &p.version.path, &t.scope, &mut metrics)? == p.current,
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
        Ok(())
    }
}
