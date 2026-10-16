use crate::*;
use anyhow::{Context, Result, ensure};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::atomic::Ordering,
};
#[cfg(test)]
thread_local! { static FAIL_STAGE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
impl Engine {
    pub(crate) fn transfer(
        &self,
        source: &Path,
        target: &Path,
        digest: &str,
        expected_destination: Option<&str>,
        t: &Task,
        m: &mut Metrics,
    ) -> Result<()> {
        let parent = target.parent().context("Missing target parent")?;
        let parent_identity = crate::fs::native_file_id(parent)?;
        // Capture destination content before any potentially slow staging work.
        let destination_before = match fs::symlink_metadata(target) {
            Ok(metadata) => {
                ensure!(
                    !metadata.is_symlink() && metadata.is_file(),
                    "Unsafe destination type"
                );
                Some(crate::fs::hash_file(target, m)?.0)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        ensure!(
            destination_before.as_deref() == expected_destination,
            "Destination changed after review/backup"
        );
        let stage = target
            .parent()
            .context("Missing target parent")?
            .join(format!(".foldersync-stage-{}", uuid::Uuid::new_v4()));
        self.index.db.execute(
            "INSERT INTO staging VALUES(?,?)",
            rusqlite::params![stage.to_string_lossy(), t.id],
        )?;
        let result = (|| -> Result<()> {
            let mut options = OpenOptions::new();
            options.read(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                options.share_mode(5);
            }
            let mut input = options.open(source)?;
            let before = input.metadata()?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&stage)?;
            let start = std::time::Instant::now();
            let mut count = 0;
            let mut hash = blake3::Hasher::new();
            let mut buf = vec![0; 256 * 1024];
            loop {
                ensure!(
                    !self.cancel.load(Ordering::Relaxed),
                    "Cancelled during staged transfer"
                );
                let n = input.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                #[cfg(test)]
                if FAIL_STAGE.get() && count >= 256 * 1024 {
                    return Err(std::io::Error::from(std::io::ErrorKind::StorageFull).into());
                }
                output.write_all(&buf[..n])?;
                hash.update(&buf[..n]);
                count += n as u64;
                if t.bandwidth_kib > 0 {
                    let expected = std::time::Duration::from_secs_f64(
                        count as f64 / (t.bandwidth_kib * 1024) as f64,
                    );
                    if let Some(mut delay) = expected.checked_sub(start.elapsed()) {
                        while !delay.is_zero() {
                            ensure!(!self.cancel.load(Ordering::Relaxed), "Cancelled");
                            let chunk = delay.min(std::time::Duration::from_millis(100));
                            std::thread::sleep(chunk);
                            delay = delay.saturating_sub(chunk);
                        }
                    }
                }
            }
            output.sync_all()?;
            drop(output);
            ensure!(
                hash.finalize().to_hex().as_str() == digest && count == before.len(),
                "Transfer verification failed"
            );
            ensure!(
                input.metadata()?.modified()? == before.modified()?,
                "Source changed during transfer"
            );
            ensure!(
                crate::fs::hash_file(&stage, m)?.0 == digest,
                "Staged content differs"
            );
            filetime::set_file_mtime(
                &stage,
                filetime::FileTime::from_last_modification_time(&before),
            )?;
            #[cfg(windows)]
            let _destination_guard = if expected_destination.is_some() {
                use std::os::windows::fs::OpenOptionsExt;
                Some(OpenOptions::new().read(true).share_mode(5).open(target)?)
            } else {
                None
            };
            let destination_now = match fs::symlink_metadata(target) {
                Ok(metadata) => {
                    ensure!(
                        !metadata.is_symlink() && metadata.is_file(),
                        "Destination type changed"
                    );
                    Some(crate::fs::hash_file(target, m)?.0)
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.into()),
            };
            ensure!(
                destination_now == destination_before,
                "Destination changed during staging; preserving both versions"
            );
            ensure!(
                crate::fs::native_file_id(parent)? == parent_identity,
                "Destination parent changed during staging"
            );
            for root in [&t.a, &t.b, self.storage()] {
                if let Ok(relative) = target.strip_prefix(root) {
                    crate::fs::safe_path(root, &relative.to_string_lossy())?;
                    break;
                }
            }
            #[cfg(windows)]
            {
                use std::os::windows::ffi::OsStrExt;
                use windows_sys::Win32::Storage::FileSystem::{
                    MOVEFILE_WRITE_THROUGH, MoveFileExW, ReplaceFileW,
                };
                let source_absolute = fs::canonicalize(&stage)?;
                let target_absolute = fs::canonicalize(parent)?
                    .join(target.file_name().context("Missing destination filename")?);
                let wide = |path: &Path| {
                    path.as_os_str()
                        .encode_wide()
                        .chain(Some(0))
                        .collect::<Vec<_>>()
                };
                let a = wide(&source_absolute);
                let b = wide(&target_absolute);
                if expected_destination.is_some() {
                    // ReplaceFile captures the actual displaced object, including a
                    // rename racing the final check. Journal before touching either name.
                    let displaced = target_absolute
                        .with_file_name(format!(".foldersync-displaced-{}", uuid::Uuid::new_v4()));
                    let (side, relative) = if let Ok(p) = target.strip_prefix(&t.a) {
                        ("A", p)
                    } else {
                        ("B", target.strip_prefix(&t.b)?)
                    };
                    self.index.db.execute(
                        "INSERT INTO displaced VALUES(?,?,?,?)",
                        rusqlite::params![
                            displaced.to_string_lossy(),
                            t.id,
                            relative.to_string_lossy(),
                            side
                        ],
                    )?;
                    let c = wide(&displaced);
                    let ok = unsafe {
                        ReplaceFileW(
                            b.as_ptr(),
                            a.as_ptr(),
                            c.as_ptr(),
                            0,
                            std::ptr::null(),
                            std::ptr::null(),
                        )
                    };
                    let error = std::io::Error::last_os_error();
                    let preserved =
                        self.preserve_displaced(&displaced, t, &relative.to_string_lossy(), side)?;
                    ensure!(ok != 0, "Atomic replacement failed: {error}");
                    ensure!(
                        preserved.as_deref() == expected_destination,
                        "Destination raced commit; displaced bytes preserved in protected history; review again"
                    );
                } else {
                    // Never replace a file which appeared after an absence check.
                    ensure!(
                        unsafe { MoveFileExW(a.as_ptr(), b.as_ptr(), MOVEFILE_WRITE_THROUGH) } != 0,
                        "Atomic creation failed: {}",
                        std::io::Error::last_os_error()
                    );
                }
            }
            #[cfg(not(windows))]
            fs::rename(&stage, target)?;
            m.copied_bytes += count;
            Ok(())
        })();
        if stage.exists() {
            let _ = fs::remove_file(&stage);
        }
        if !stage.exists() {
            self.index.db.execute(
                "DELETE FROM staging WHERE path=?",
                [stage.to_string_lossy().as_ref()],
            )?;
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn injected_disk_full_keeps_destination_and_cleans_owned_stage() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("A");
        let b = d.path().join("B");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        let mut e = Engine::open(&d.path().join("state")).unwrap();
        let t = Task {
            a,
            b,
            ..Task::default()
        };
        e.save_task(&t).unwrap();
        let source = t.a.join("f");
        let target = t.b.join("f");
        fs::write(&source, vec![42; 1024 * 1024]).unwrap();
        fs::write(&target, "old precious").unwrap();
        let mut m = Metrics::default();
        let old = crate::fs::hash_file(&target, &mut m).unwrap().0;
        let digest = crate::fs::hash_file(&source, &mut m).unwrap().0;
        FAIL_STAGE.set(true);
        let result = e.transfer(&source, &target, &digest, Some(&old), &t, &mut m);
        FAIL_STAGE.set(false);
        assert!(result.unwrap_err().to_string().contains("space"));
        assert_eq!(fs::read(&target).unwrap(), b"old precious");
        assert_eq!(fs::read_dir(&t.b).unwrap().count(), 1);
    }
    #[test]
    fn expected_destination_is_not_recaptured_after_backup() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("A");
        let b = d.path().join("B");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        let mut e = Engine::open(&d.path().join("state")).unwrap();
        let t = Task {
            a,
            b,
            ..Task::default()
        };
        e.save_task(&t).unwrap();
        let source = t.a.join("f");
        let target = t.b.join("f");
        fs::write(&source, "source").unwrap();
        fs::write(&target, "reviewed").unwrap();
        let mut m = Metrics::default();
        let expected = crate::fs::hash_file(&target, &mut m).unwrap().0;
        let digest = crate::fs::hash_file(&source, &mut m).unwrap().0;
        fs::write(&target, "external after backup").unwrap();
        assert!(
            e.transfer(&source, &target, &digest, Some(&expected), &t, &mut m)
                .is_err()
        );
        assert_eq!(fs::read(target).unwrap(), b"external after backup");
    }
    #[test]
    fn interrupted_atomic_displacement_is_preserved_on_restart() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("A");
        let b = d.path().join("B");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        let state = d.path().join("state");
        let mut e = Engine::open(&state).unwrap();
        let t = Task {
            a,
            b,
            ..Task::default()
        };
        e.save_task(&t).unwrap();
        let displaced = fs::canonicalize(&t.b)
            .unwrap()
            .join(format!(".foldersync-displaced-{}", uuid::Uuid::new_v4()));
        fs::write(&displaced, "racing precious bytes").unwrap();
        e.index
            .db
            .execute(
                "INSERT INTO displaced VALUES(?,?,?,?)",
                rusqlite::params![displaced.to_string_lossy(), t.id, "f", "B"],
            )
            .unwrap();
        drop(e);
        let e = Engine::open(&state).unwrap();
        assert!(!displaced.exists());
        let v = e.versions(&t.id).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(fs::read(&v[0].stored).unwrap(), b"racing precious bytes");
    }
}
