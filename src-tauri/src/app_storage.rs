use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags, params};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

const DATABASE: &str = "foldersync.sqlite3";
const COMPLETE: &str = ".foldersync-storage-v1";

/// Keep the human-facing storage name independent of the bundle identifier.
/// Migration is a one-time switch: the legacy folder is retained for recovery,
/// and later changes made by an old release are never merged into the new state.
pub fn resolve_storage(identifier_data_dir: &Path) -> Result<PathBuf> {
    resolve_at(
        identifier_data_dir
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Missing application data parent"))?,
        ensure_old_app_closed,
    )
}

/// Explicit uninstall cleanup of the two owned application-state directories.
/// Synchronization roots and any other directories are never cleanup targets.
pub fn cleanup_storage(parent: &Path) -> Result<()> {
    cleanup_at(parent, ensure_old_app_closed)
}

fn cleanup_at(parent: &Path, check_old_app: impl FnOnce() -> Result<()>) -> Result<()> {
    reject_reparse_ancestors(parent)?;
    let mut targets = Vec::new();
    for name in ["FolderSync", "dev.foldersync.desktop"] {
        let target = parent.join(name);
        match fs::symlink_metadata(&target) {
            Ok(metadata) => {
                ensure!(
                    metadata.is_dir() && !is_reparse(&metadata),
                    "Unsafe application storage cleanup target: {}",
                    target.display()
                );
                // Inspect every target before deleting either one: an invalid
                // legacy directory must not cause partial uninstall cleanup.
                inspect_tree(&target)?;
                validate_database(&target)?;
                let db = Connection::open_with_flags(
                    target.join(DATABASE),
                    OpenFlags::SQLITE_OPEN_READ_ONLY,
                )?;
                let schema: u32 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
                ensure!(
                    (1..=5).contains(&schema),
                    "Unrecognized application storage schema; refusing cleanup"
                );
                db.prepare("SELECT id,json,root_a,root_b,generation FROM tasks LIMIT 0")?;
                db.prepare("SELECT id,task,json FROM versions LIMIT 0")?;
                drop(db);
                targets.push(target);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    if targets.is_empty() {
        return Ok(());
    }
    check_old_app()?;
    for target in targets {
        // These paths are constructed exclusively from the validated parent
        // and the two literal directory names above, never database paths.
        fs::remove_dir_all(&target)
            .with_context(|| format!("Cannot remove application storage {}", target.display()))?;
    }
    Ok(())
}

fn resolve_at(parent: &Path, check_old_app: impl FnOnce() -> Result<()>) -> Result<PathBuf> {
    let target = parent.join("FolderSync");
    // This historical identifier is required to recover existing installations.
    let source = parent.join("dev.foldersync.desktop");
    let stage = parent.join(".FolderSync.migrating");
    reject_reparse_ancestors(parent)?;
    ensure!(
        !stage.try_exists()?,
        "Incomplete FolderSync migration exists at {}; preserve it and the legacy folder before retrying",
        stage.display()
    );
    if target.try_exists()? {
        reject_reparse_ancestors(&target)?;
        ensure!(target.is_dir(), "FolderSync storage must be a directory");
        if source.try_exists()? {
            check_old_app()?;
            ensure!(
                target.join(COMPLETE).is_file(),
                "Both legacy storage and an unrecognized FolderSync target exist; refusing to merge application state"
            );
        }
        validate_database(&target)?;
        return Ok(target);
    }
    if !source.try_exists()? {
        return Ok(target);
    }
    check_old_app()?;
    reject_reparse_ancestors(&source)?;
    ensure!(
        source.is_dir(),
        "Legacy application storage must be a directory"
    );
    inspect_tree(&source)?;
    ensure!(
        source.join(DATABASE).is_file(),
        "Legacy application database is missing; refusing partial migration"
    );
    // Hold a write reservation throughout both the SQLite snapshot and file copy.
    // A separate connection can read the committed WAL while all writers wait.
    let reservation =
        Connection::open_with_flags(source.join(DATABASE), OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    reservation.busy_timeout(std::time::Duration::from_secs(1))?;
    reservation
        .execute_batch("BEGIN IMMEDIATE")
        .context("Legacy database is busy; close FolderSync before migrating")?;
    fs::create_dir(&stage)?;
    let reader =
        Connection::open_with_flags(source.join(DATABASE), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    reader.execute(
        "VACUUM main INTO ?",
        [stage.join(DATABASE).to_string_lossy().as_ref()],
    )?;
    drop(reader);
    copy_tree(&source, &stage, true)?;
    relocate_database(&stage, &source, &target)?;
    validate_database(&stage)?;
    let mut marker = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(stage.join(COMPLETE))?;
    use std::io::Write;
    marker.write_all(b"FolderSync storage migration complete\nLegacy storage is retained for recovery only. Later legacy changes are not merged. Do not run old FolderSync releases after switching.\n")?;
    marker.sync_all()?;
    drop(marker);
    ensure!(
        !target.try_exists()?,
        "FolderSync target appeared during migration; refusing to overwrite it"
    );
    fs::rename(&stage, &target)
        .context("Cannot publish migrated FolderSync storage; legacy source is preserved")?;
    reservation.execute_batch("ROLLBACK")?;
    Ok(target)
}

fn is_reparse(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.is_symlink()
    }
}

fn reject_reparse_ancestors(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        let m = fs::symlink_metadata(ancestor)?;
        ensure!(
            !is_reparse(&m),
            "Reparse points are unsupported in application storage: {}",
            ancestor.display()
        );
    }
    Ok(())
}

fn inspect_tree(path: &Path) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let m = fs::symlink_metadata(entry.path())?;
        ensure!(
            !is_reparse(&m),
            "Reparse points are unsupported in application storage: {}",
            entry.path().display()
        );
        ensure!(
            m.is_dir() || m.is_file(),
            "Unsupported application storage object"
        );
        if m.is_dir() {
            inspect_tree(&entry.path())?;
        }
    }
    Ok(())
}

fn copy_tree(source: &Path, target: &Path, top: bool) -> Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        let m = fs::symlink_metadata(entry.path())?;
        ensure!(!is_reparse(&m), "Reparse point appeared during migration");
        if top
            && matches!(
                name.to_str(),
                Some(
                    DATABASE
                        | "foldersync.sqlite3-wal"
                        | "foldersync.sqlite3-shm"
                        | "foldersync.sqlite3-journal"
                )
            )
        {
            continue;
        }
        let destination = target.join(&name);
        if m.is_dir() {
            fs::create_dir(&destination)?;
            copy_tree(&entry.path(), &destination, false)?;
        } else {
            ensure!(m.is_file(), "Unsupported application storage object");
            fs::copy(entry.path(), &destination)?;
            fs::OpenOptions::new()
                .write(true)
                .open(destination)?
                .sync_all()?;
        }
    }
    Ok(())
}

fn relocated(path: &Path, source: &Path, target: &Path) -> Result<PathBuf> {
    match path.strip_prefix(source) {
        Ok(relative) => {
            ensure!(
                relative
                    .components()
                    .all(|c| matches!(c, Component::Normal(_))),
                "Unsafe internal migration path"
            );
            Ok(target.join(relative))
        }
        Err(_) => Ok(path.to_path_buf()),
    }
}

fn protected_relative(
    version: &foldersync_engine::Version,
    source: &Path,
) -> Result<(PathBuf, PathBuf)> {
    uuid::Uuid::parse_str(&version.id).context("Invalid protected version UUID")?;
    uuid::Uuid::parse_str(&version.task_id).context("Invalid protected task UUID")?;
    let flat = PathBuf::from("versions").join(&version.id);
    let nested = PathBuf::from("versions")
        .join(&version.task_id)
        .join(&version.id);
    ensure!(
        version.stored == source.join(&flat) || version.stored == source.join(&nested),
        "Unexpected protected version catalog path"
    );
    let original = version.stored.strip_prefix(source)?.to_path_buf();
    ensure!(
        original
            .components()
            .all(|c| matches!(c, Component::Normal(_))),
        "Unsafe protected version path"
    );
    Ok((original, nested))
}

fn relocate_database(stage: &Path, source: &Path, target: &Path) -> Result<()> {
    use foldersync_engine::{Metrics, RestorePreview, Version};
    let db = Connection::open(stage.join(DATABASE))?;
    db.execute_batch("PRAGMA synchronous=FULL; BEGIN IMMEDIATE")?;
    let versions: Vec<(String, String, String)> = db
        .prepare("SELECT id,task,json FROM versions")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut moved_versions = std::collections::HashMap::new();
    for (id, task, json) in versions {
        let mut v: Version = serde_json::from_str(&json)?;
        ensure!(
            v.id == id && v.task_id == task,
            "Protected version catalog identity mismatch"
        );
        let (relative, nested) = protected_relative(&v, source)?;
        let checked = foldersync_engine::fs::safe_path(stage, &relative.to_string_lossy())?;
        ensure!(
            foldersync_engine::fs::hash_file(&checked, &mut Metrics::default())?.0 == v.digest,
            "Protected version failed integrity validation during migration"
        );
        let destination = foldersync_engine::fs::safe_path(stage, &nested.to_string_lossy())?;
        if checked != destination {
            ensure!(
                !destination.try_exists()?,
                "Ambiguous protected version migration destination"
            );
            fs::create_dir_all(
                destination
                    .parent()
                    .context("Missing protected version parent")?,
            )?;
            fs::rename(&checked, &destination)?;
        }
        v.stored = target.join(nested);
        moved_versions.insert(id.clone(), v.clone());
        db.execute(
            "UPDATE versions SET json=? WHERE id=?",
            params![serde_json::to_string(&v)?, id],
        )?;
    }
    for table in ["staging", "displaced"] {
        let paths: Vec<String> = db
            .prepare(&format!("SELECT path FROM {table}"))?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for path in paths {
            let moved = relocated(Path::new(&path), source, target)?;
            db.execute(
                &format!("UPDATE {table} SET path=? WHERE path=?"),
                params![moved.to_string_lossy(), path],
            )?;
        }
    }
    let previews: Vec<(String, String)> = db
        .prepare("SELECT key,json FROM settings WHERE key LIKE 'restore/%'")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (key, json) in previews {
        let mut preview: RestorePreview = serde_json::from_str(&json)?;
        let (_, nested) = protected_relative(&preview.version, source)?;
        if let Some(version) = moved_versions.get(&preview.version.id) {
            ensure!(
                preview.version.task_id == version.task_id
                    && preview.version.digest == version.digest,
                "Restore preview protected identity mismatch"
            );
            preview.version.stored = version.stored.clone();
        } else {
            // Retention may already have expired this preview's version. Keep
            // it invalid rather than reconnecting it to any other file.
            preview.version.stored = target.join(nested);
        }
        db.execute(
            "UPDATE settings SET json=? WHERE key=?",
            params![serde_json::to_string(&preview)?, key],
        )?;
    }
    db.execute_batch("COMMIT; PRAGMA wal_checkpoint(TRUNCATE);")?;
    drop(db);
    fs::OpenOptions::new()
        .write(true)
        .open(stage.join(DATABASE))?
        .sync_all()?;
    Ok(())
}

fn validate_database(storage: &Path) -> Result<()> {
    reject_reparse_ancestors(&storage.join(DATABASE))?;
    let db = Connection::open_with_flags(storage.join(DATABASE), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    ensure!(
        db.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0))? == "ok",
        "Application database failed integrity validation"
    );
    ensure!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))? <= 5,
        "Application database is newer than this version"
    );
    Ok(())
}

fn ensure_old_app_closed() -> Result<()> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
            System::Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
        };
        // Snapshot handle is always closed, including a failed enumeration.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        ensure!(
            snapshot != INVALID_HANDLE_VALUE,
            "Cannot check running FolderSync processes; close FolderSync before migrating"
        );
        let result = (|| -> Result<()> {
            let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            ensure!(
                unsafe { Process32FirstW(snapshot, &mut entry) } != 0,
                "Cannot enumerate running processes before migration"
            );
            loop {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
                ensure!(
                    !(entry.th32ProcessID != std::process::id()
                        && name.eq_ignore_ascii_case("foldersync.exe")),
                    "Close FolderSync before migrating application storage"
                );
                if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
                    break;
                }
            }
            Ok(())
        })();
        unsafe {
            CloseHandle(snapshot);
        }
        result?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use foldersync_engine::{Engine, Task};
    use rusqlite::{Connection, params};
    use std::fs;

    fn fixture(parent: &Path) -> (Engine, Task) {
        let a = parent.join("a");
        let b = parent.join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        let mut e = Engine::open(&parent.join("dev.foldersync.desktop")).unwrap();
        let t = Task {
            a,
            b,
            ..Task::default()
        };
        e.save_task(&t).unwrap();
        for text in ["original", "updated"] {
            fs::write(t.a.join("f"), text).unwrap();
            let p = e.preview(&t.id).unwrap();
            e.apply(&p.id, true).unwrap();
        }
        (e, t)
    }

    #[test]
    fn explicit_cleanup_removes_both_storage_directories_and_preserves_sync_roots() {
        let d = tempfile::tempdir().unwrap();
        let (e, t) = fixture(d.path());
        resolve_at(d.path(), || Ok(())).unwrap();
        drop(e);
        let unrelated = d.path().join("unrelated");
        fs::create_dir(&unrelated).unwrap();
        fs::write(unrelated.join("keep"), "outside data").unwrap();
        cleanup_at(d.path(), || Ok(())).unwrap();
        assert!(!d.path().join("FolderSync").exists());
        assert!(!d.path().join("dev.foldersync.desktop").exists());
        assert_eq!(fs::read(t.a.join("f")).unwrap(), b"updated");
        assert_eq!(fs::read(t.b.join("f")).unwrap(), b"updated");
        assert_eq!(fs::read(unrelated.join("keep")).unwrap(), b"outside data");
        cleanup_at(d.path(), || panic!("empty cleanup must be idempotent")).unwrap();
    }

    #[test]
    fn explicit_cleanup_validates_all_storage_before_deleting_either() {
        let d = tempfile::tempdir().unwrap();
        let current = Engine::open(&d.path().join("FolderSync")).unwrap();
        drop(current);
        let legacy = d.path().join("dev.foldersync.desktop");
        fs::create_dir(&legacy).unwrap();
        let invalid = Connection::open(legacy.join(DATABASE)).unwrap();
        invalid
            .execute_batch("PRAGMA user_version=5; CREATE TABLE unrelated(value TEXT);")
            .unwrap();
        drop(invalid);
        assert!(cleanup_at(d.path(), || Ok(())).is_err());
        assert!(d.path().join("FolderSync").join(DATABASE).is_file());
        assert!(legacy.join(DATABASE).is_file());
    }

    #[test]
    fn explicit_cleanup_refuses_running_app_without_deleting_storage() {
        let d = tempfile::tempdir().unwrap();
        let (e, _t) = fixture(d.path());
        resolve_at(d.path(), || Ok(())).unwrap();
        drop(e);
        assert!(
            cleanup_at(d.path(), || anyhow::bail!(
                "Close FolderSync before removing application storage"
            ))
            .is_err()
        );
        assert!(d.path().join("FolderSync").join(DATABASE).is_file());
        assert!(
            d.path()
                .join("dev.foldersync.desktop")
                .join(DATABASE)
                .is_file()
        );
    }

    #[cfg(windows)]
    #[test]
    fn explicit_cleanup_rejects_nested_junction_before_deleting_any_storage() {
        let d = tempfile::tempdir().unwrap();
        let (e, t) = fixture(d.path());
        resolve_at(d.path(), || Ok(())).unwrap();
        drop(e);
        let link = d.path().join("dev.foldersync.desktop").join("junction");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&t.a)
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
        assert!(cleanup_at(d.path(), || Ok(())).is_err());
        assert!(d.path().join("FolderSync").join(DATABASE).is_file());
        assert!(
            d.path()
                .join("dev.foldersync.desktop")
                .join(DATABASE)
                .is_file()
        );
        assert_eq!(fs::read(t.a.join("f")).unwrap(), b"updated");
        fs::remove_dir(link).unwrap();
    }

    #[test]
    fn migration_preserves_wal_baselines_versions_and_reviewed_restore() {
        let d = tempfile::tempdir().unwrap();
        let (e, t) = fixture(d.path());
        let source = d.path().join("dev.foldersync.desktop");
        let mut v = e.versions(&t.id).unwrap().remove(0);
        let flat = source.join("versions").join(&v.id);
        fs::rename(&v.stored, &flat).unwrap();
        v.stored = flat;
        let db = Connection::open(source.join(DATABASE)).unwrap();
        db.execute(
            "UPDATE versions SET json=? WHERE id=?",
            params![serde_json::to_string(&v).unwrap(), v.id],
        )
        .unwrap();
        let restore = e.restore_preview(&v.id).unwrap();
        // Connection remains open; committed changes really are in WAL.
        assert!(source.join("foldersync.sqlite3-wal").exists());
        let target = resolve_at(d.path(), || Ok(())).unwrap();
        assert_eq!(target, d.path().join("FolderSync"));
        assert!(v.stored.exists(), "legacy source must remain intact");
        let mut migrated = Engine::open(&target).unwrap();
        assert_eq!(migrated.tasks().unwrap().len(), 1);
        assert!(
            migrated
                .preview(&t.id)
                .unwrap()
                .operations
                .iter()
                .all(|op| op.action == foldersync_engine::Action::Unchanged)
        );
        let moved = migrated.versions(&t.id).unwrap().remove(0);
        assert_eq!(
            moved.stored,
            target.join("versions").join(&t.id).join(&v.id)
        );
        assert!(!target.join("versions").join(&v.id).exists());
        assert_eq!(fs::read(&moved.stored).unwrap(), b"original");
        migrated.restore_apply(&restore.id).unwrap();
        assert_eq!(fs::read(t.b.join("f")).unwrap(), b"original");
        assert_eq!(resolve_at(d.path(), || Ok(())).unwrap(), target);
    }

    #[test]
    fn completed_migration_still_rejects_a_running_legacy_process() {
        let d = tempfile::tempdir().unwrap();
        let (_e, _t) = fixture(d.path());
        let target = resolve_at(d.path(), || Ok(())).unwrap();
        let error = resolve_at(d.path(), || {
            anyhow::bail!("Close FolderSync before migrating application storage")
        })
        .unwrap_err();
        assert!(error.to_string().contains("Close FolderSync"));
        assert!(target.join(DATABASE).is_file());
        assert!(
            d.path()
                .join("dev.foldersync.desktop")
                .join(DATABASE)
                .is_file()
        );
    }

    #[test]
    fn refuses_ambiguous_target_incomplete_stage_and_running_old_process() {
        let d = tempfile::tempdir().unwrap();
        let (_e, _t) = fixture(d.path());
        fs::create_dir(d.path().join("FolderSync")).unwrap();
        assert!(resolve_at(d.path(), || Ok(())).is_err());
        fs::remove_dir(d.path().join("FolderSync")).unwrap();
        fs::create_dir(d.path().join(".FolderSync.migrating")).unwrap();
        assert!(resolve_at(d.path(), || Ok(())).is_err());
        fs::remove_dir(d.path().join(".FolderSync.migrating")).unwrap();
        assert!(
            resolve_at(d.path(), || anyhow::bail!(
                "Close FolderSync before migrating"
            ))
            .is_err()
        );
        assert!(!d.path().join("FolderSync").exists());
    }

    #[test]
    fn migration_refuses_missing_protected_file_without_publishing_target() {
        let d = tempfile::tempdir().unwrap();
        let (e, t) = fixture(d.path());
        fs::remove_file(e.versions(&t.id).unwrap()[0].stored.clone()).unwrap();
        assert!(resolve_at(d.path(), || Ok(())).is_err());
        assert!(!d.path().join("FolderSync").exists());
    }

    #[test]
    fn migration_keeps_archived_flat_legacy_versions_restorable() {
        let d = tempfile::tempdir().unwrap();
        let (mut e, t) = fixture(d.path());
        let mut v = e.versions(&t.id).unwrap().remove(0);
        let flat = e.storage().join("versions").join(&v.id);
        fs::rename(&v.stored, &flat).unwrap();
        v.stored = flat;
        let db = Connection::open(e.storage().join(DATABASE)).unwrap();
        db.execute(
            "UPDATE versions SET json=? WHERE id=?",
            params![serde_json::to_string(&v).unwrap(), v.id],
        )
        .unwrap();
        e.delete_task(&t.id).unwrap();
        let target = resolve_at(d.path(), || Ok(())).unwrap();
        let mut migrated = Engine::open(&target).unwrap();
        assert!(migrated.tasks().unwrap().is_empty());
        assert_eq!(migrated.recovery_tasks().unwrap()[0].task.id, t.id);
        let restored = migrated.versions(&t.id).unwrap().remove(0);
        assert_eq!(
            restored.stored,
            target.join("versions").join(&t.id).join(&v.id)
        );
        let preview = migrated.restore_preview(&v.id).unwrap();
        migrated.restore_apply(&preview.id).unwrap();
        assert_eq!(fs::read(t.b.join("f")).unwrap(), b"original");
    }

    #[test]
    fn migration_refuses_busy_source_before_creating_stage() {
        let d = tempfile::tempdir().unwrap();
        let (_e, _t) = fixture(d.path());
        let db = Connection::open(d.path().join("dev.foldersync.desktop").join(DATABASE)).unwrap();
        db.execute_batch("BEGIN IMMEDIATE").unwrap();
        let error = resolve_at(d.path(), || Ok(())).unwrap_err();
        assert!(error.to_string().contains("busy"));
        assert!(!d.path().join(".FolderSync.migrating").exists());
        assert!(!d.path().join("FolderSync").exists());
    }

    #[test]
    fn migration_rejects_unexpected_protected_catalog_path() {
        let d = tempfile::tempdir().unwrap();
        let (e, t) = fixture(d.path());
        let mut v = e.versions(&t.id).unwrap().remove(0);
        let unexpected = e.storage().join("unrelated-protected-file");
        fs::rename(&v.stored, &unexpected).unwrap();
        v.stored = unexpected.clone();
        let db = Connection::open(e.storage().join(DATABASE)).unwrap();
        db.execute(
            "UPDATE versions SET json=? WHERE id=?",
            params![serde_json::to_string(&v).unwrap(), v.id],
        )
        .unwrap();
        assert!(resolve_at(d.path(), || Ok(())).is_err());
        assert!(!d.path().join("FolderSync").exists());
        assert_eq!(fs::read(&unexpected).unwrap(), b"original");
    }

    #[test]
    fn migration_relocates_owned_staging_paths_but_preserves_external_paths() {
        let d = tempfile::tempdir().unwrap();
        let (e, t) = fixture(d.path());
        let source = d.path().join("dev.foldersync.desktop");
        let db = Connection::open(source.join("foldersync.sqlite3")).unwrap();
        let stage = source.join("versions/.foldersync-stage-fixture");
        fs::write(&stage, "partial").unwrap();
        db.execute(
            "INSERT INTO staging VALUES(?,?)",
            params![stage.to_string_lossy(), t.id],
        )
        .unwrap();
        let external = t.a.join(".foldersync-stage-external");
        db.execute(
            "INSERT INTO staging VALUES(?,?)",
            params![external.to_string_lossy(), t.id],
        )
        .unwrap();
        let target = resolve_at(d.path(), || Ok(())).unwrap();
        let moved = Connection::open(target.join("foldersync.sqlite3")).unwrap();
        let paths: Vec<String> = moved
            .prepare("SELECT path FROM staging ORDER BY path")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(
            paths.contains(
                &target
                    .join("versions/.foldersync-stage-fixture")
                    .to_string_lossy()
                    .into_owned()
            )
        );
        assert!(paths.contains(&external.to_string_lossy().into_owned()));
        drop(e);
    }

    #[cfg(windows)]
    #[test]
    fn migration_refuses_directory_junctions() {
        let d = tempfile::tempdir().unwrap();
        let (_e, _t) = fixture(d.path());
        let outside = d.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let link = d.path().join("dev.foldersync.desktop").join("junction");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "{} {}",
            String::from_utf8_lossy(&status.stdout),
            String::from_utf8_lossy(&status.stderr)
        );
        assert!(resolve_at(d.path(), || Ok(())).is_err());
        assert!(!d.path().join("FolderSync").exists());
        fs::remove_dir(link).unwrap();
    }
}
