use crate::*;
use anyhow::{Context, Result, ensure};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::atomic::Ordering,
};
impl Engine {
    pub(crate) fn transfer(
        &self,
        source: &Path,
        target: &Path,
        digest: &str,
        t: &Task,
        m: &mut Metrics,
    ) -> Result<()> {
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
                options.share_mode(1);
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
            #[cfg(windows)]
            {
                use std::os::windows::ffi::OsStrExt;
                use windows_sys::Win32::Storage::FileSystem::{
                    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
                };
                let a: Vec<_> = stage.as_os_str().encode_wide().chain(Some(0)).collect();
                let b: Vec<_> = target.as_os_str().encode_wide().chain(Some(0)).collect();
                ensure!(
                    unsafe {
                        MoveFileExW(
                            a.as_ptr(),
                            b.as_ptr(),
                            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                        )
                    } != 0,
                    "Atomic replacement failed: {}",
                    std::io::Error::last_os_error()
                );
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
