use crate::Runtime;
use serde::Serialize;
use std::{
    sync::{Mutex, TryLockError, atomic::Ordering},
    time::Duration,
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Clone, Serialize)]
pub struct Offer {
    pub version: String,
    pub notes: String,
}
#[derive(Clone, Serialize)]
pub struct Status {
    pub current_version: String,
    pub available: Option<Offer>,
    pub checking: bool,
    pub installing: bool,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub error: Option<String>,
    pub checked: bool,
}
struct Inner {
    status: Status,
    update: Option<Update>,
}
pub struct Updates(Mutex<Inner>);
impl Default for Updates {
    fn default() -> Self {
        Self(Mutex::new(Inner {
            status: Status {
                current_version: env!("CARGO_PKG_VERSION").into(),
                available: None,
                checking: false,
                installing: false,
                downloaded: 0,
                total: None,
                error: None,
                checked: false,
            },
            update: None,
        }))
    }
}
fn publish(app: &tauri::AppHandle) {
    if let Ok(inner) = app.state::<Updates>().0.lock() {
        let _ = app.emit("update-status", inner.status.clone());
    }
}

pub fn validate_source(version: &str, url: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        url == format!(
            "https://github.com/cancel-cloud/FolderSync/releases/download/v{version}/FolderSync_{version}_x64-setup.exe"
        ),
        "Update is not an official FolderSync release asset"
    );
    Ok(())
}

#[tauri::command]
pub fn update_status(state: State<'_, Updates>) -> Result<Status, String> {
    state
        .0
        .lock()
        .map(|i| i.status.clone())
        .map_err(|_| "Update state unavailable".into())
}

#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<Status, String> {
    {
        let state = app.state::<Updates>();
        let mut inner = state.0.lock().map_err(|_| "Update state unavailable")?;
        if inner.status.checking || inner.status.installing {
            return Ok(inner.status.clone());
        }
        inner.status.checking = true;
        inner.status.error = None;
    }
    publish(&app);
    let result = async {
        let update = app
            .updater_builder()
            .timeout(Duration::from_secs(20))
            .build()?
            .check()
            .await?;
        if let Some(ref update) = update {
            validate_source(&update.version, update.download_url.as_str())?;
        }
        Ok::<_, anyhow::Error>(update)
    }
    .await;
    let status = {
        let state = app.state::<Updates>();
        let mut inner = state.0.lock().map_err(|_| "Update state unavailable")?;
        inner.status.checking = false;
        inner.status.checked = true;
        match result {
            Ok(update) => {
                inner.status.available = update.as_ref().map(|u| Offer {
                    version: u.version.clone(),
                    notes: u
                        .body
                        .as_deref()
                        .unwrap_or("")
                        .chars()
                        .take(65536)
                        .collect(),
                });
                inner.update = update;
            }
            Err(error) => inner.status.error = Some(format!("Update check failed: {error:#}")),
        }
        inner.status.clone()
    };
    publish(&app);
    Ok(status)
}

fn requested_version_matches(
    requested: &str,
    available: Option<&str>,
    installing: bool,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !installing,
        "An update check or installation is already running"
    );
    anyhow::ensure!(
        available == Some(requested),
        "The offered update changed; check updates again before installing"
    );
    Ok(())
}

#[tauri::command]
pub async fn install_update(
    app: tauri::AppHandle,
    runtime: State<'_, Runtime>,
    version: String,
) -> Result<(), String> {
    let mut update = {
        let state = app.state::<Updates>();
        let mut inner = state.0.lock().map_err(|_| "Update state unavailable")?;
        requested_version_matches(
            &version,
            inner.update.as_ref().map(|u| u.version.as_str()),
            inner.status.installing || inner.status.checking,
        )
        .map_err(|e| e.to_string())?;
        let update = inner.update.clone().ok_or("No update available")?;
        inner.status.installing = true;
        inner.status.error = None;
        inner.status.downloaded = 0;
        inner.status.total = None;
        update
    };
    publish(&app);
    update.timeout = Some(Duration::from_secs(300));
    let result = async {
        validate_source(&update.version, update.download_url.as_str())?;
        anyhow::ensure!(
            runtime.progress.snapshot().is_none(),
            "Finish active synchronization before installing an update"
        );
        let mut last_emit = std::time::Instant::now();
        let bytes = update
            .download(
                |chunk, total| {
                    if let Ok(mut inner) = app.state::<Updates>().0.lock() {
                        inner.status.downloaded =
                            inner.status.downloaded.saturating_add(chunk as u64);
                        inner.status.total = total;
                    }
                    if last_emit.elapsed() >= Duration::from_millis(200) {
                        publish(&app);
                        last_emit = std::time::Instant::now();
                    }
                },
                || {},
            )
            .await?;
        publish(&app);
        install_when_idle(&runtime, || {
            update.install(bytes).map_err(anyhow::Error::from)
        })
    }
    .await;
    if let Err(error) = result {
        if let Ok(mut inner) = app.state::<Updates>().0.lock() {
            inner.status.installing = false;
            inner.status.error = Some(format!("Update not installed: {error:#}"));
        }
        publish(&app);
        return Err(format!("Update not installed: {error:#}"));
    }
    Ok(())
}

fn install_when_idle(
    runtime: &Runtime,
    install: impl FnOnce() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    // Prevent new automatic runs, but never cancel work already using the engine.
    let was_paused = runtime.paused.swap(true, Ordering::SeqCst);
    let result = match runtime.engine.try_lock() {
        Ok(guard) => {
            let result = install();
            drop(guard);
            result
        }
        Err(TryLockError::WouldBlock) => Err(anyhow::anyhow!(
            "Finish active synchronization before installing an update"
        )),
        Err(TryLockError::Poisoned(_)) => Err(anyhow::anyhow!(
            "Engine unavailable; update was not installed"
        )),
    };
    // Successful Windows installation exits from the updater while this guard is held.
    runtime.paused.store(was_paused, Ordering::SeqCst);
    result
}

pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        loop {
            let _ = tauri::async_runtime::block_on(check_update(app.clone()));
            std::thread::park_timeout(Duration::from_secs(6 * 60 * 60));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn update_asset_is_pinned_to_repo_version_and_installer() {
        let url = "https://github.com/cancel-cloud/FolderSync/releases/download/v0.1.12/FolderSync_0.1.12_x64-setup.exe";
        assert!(validate_source("0.1.12", url).is_ok());
        assert!(validate_source("0.1.13", url).is_err());
        for unsafe_url in [
            url.replace("https:", "http:"),
            url.replace("cancel-cloud", "someone-else"),
            format!("{url}?redirect=1"),
            url.replace("github.com/", "github.com.evil/"),
            url.replace("x64-setup.exe", "script.ps1"),
        ] {
            assert!(validate_source("0.1.12", &unsafe_url).is_err());
        }
    }
    #[test]
    fn explicit_install_must_match_the_offer_and_cannot_run_twice() {
        assert!(requested_version_matches("0.1.12", Some("0.1.12"), false).is_ok());
        assert!(requested_version_matches("0.1.12", Some("0.1.13"), false).is_err());
        assert!(requested_version_matches("0.1.12", None, false).is_err());
        assert!(requested_version_matches("0.1.12", Some("0.1.12"), true).is_err());
    }
    #[test]
    fn installation_does_not_interrupt_work_and_failures_restore_pause_state() {
        use std::sync::{Arc, atomic::AtomicBool};
        let dir = tempfile::tempdir().unwrap();
        let engine = foldersync_engine::Engine::open(dir.path()).unwrap();
        let runtime = Runtime {
            cancel: engine.cancel.clone(),
            progress: engine.progress.clone(),
            close_behavior: Arc::new(crate::lifecycle::CloseBehavior::load(&engine).unwrap()),
            engine: Arc::new(Mutex::new(engine)),
            paused: Arc::new(AtomicBool::new(false)),
        };
        let guard = runtime.engine.lock().unwrap();
        assert!(install_when_idle(&runtime, || panic!("busy engine must not install")).is_err());
        assert!(!runtime.paused.load(Ordering::SeqCst));
        assert!(!runtime.cancel.load(Ordering::SeqCst));
        drop(guard);
        for paused in [false, true] {
            runtime.paused.store(paused, Ordering::SeqCst);
            assert!(
                install_when_idle(&runtime, || {
                    assert!(runtime.paused.load(Ordering::SeqCst));
                    assert!(runtime.engine.try_lock().is_err());
                    anyhow::bail!("simulated installer launch failure")
                })
                .is_err()
            );
            assert_eq!(runtime.paused.load(Ordering::SeqCst), paused);
            assert!(!runtime.cancel.load(Ordering::SeqCst));
        }
    }
    #[test]
    fn signed_payload_rejects_tampering_and_version_substitution() {
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().plugins = serde_json::from_value(serde_json::json!({
            "updater": { "pubkey": include_str!("../fixtures/updater/public.key").trim(), "requireSignedVersion": true, "endpoints": ["https://github.com/cancel-cloud/FolderSync/releases/latest/download/latest.json"] }
        })).unwrap();
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .build(context)
            .unwrap();
        let updater = app
            .updater_builder()
            .pubkey(include_str!("../fixtures/updater/public.key").trim())
            .build()
            .unwrap();
        let manifest = serde_json::json!({
            "version": "999.0.0", "url": "https://github.com/cancel-cloud/FolderSync/releases/download/v999.0.0/FolderSync_999.0.0_x64-setup.exe",
            "signature": include_str!("../fixtures/updater/payload.txt.sig").trim(),
        });
        let update = updater.restore_update(manifest.clone()).unwrap().unwrap();
        let payload = include_bytes!("../fixtures/updater/payload.txt");
        update.verify(payload).unwrap();
        let mut tampered = payload.to_vec();
        tampered[0] ^= 1;
        assert!(update.verify(&tampered).is_err());
        let mut wrong_version = manifest;
        wrong_version["version"] = "999.0.1".into();
        assert!(
            updater
                .restore_update(wrong_version)
                .unwrap()
                .unwrap()
                .verify(payload)
                .is_err()
        );
    }
    #[test]
    #[ignore = "release artifact check; requires FOLDERSYNC_RELEASE_MANIFEST and FOLDERSYNC_RELEASE_INSTALLER"]
    fn verify_packaged_release_signature_and_modified_bytes() {
        let manifest_path =
            std::env::var_os("FOLDERSYNC_RELEASE_MANIFEST").expect("manifest path required");
        let installer_path =
            std::env::var_os("FOLDERSYNC_RELEASE_INSTALLER").expect("installer path required");
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().plugins = serde_json::from_value(config["plugins"].clone()).unwrap();
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .build(context)
            .unwrap();
        // Verify any supplied published artifact; this helper never installs it.
        let updater = app
            .updater_builder()
            .version_comparator(|_, _| true)
            .build()
            .unwrap();
        let manifest = serde_json::from_slice(&std::fs::read(manifest_path).unwrap()).unwrap();
        let update = updater.restore_update(manifest).unwrap().unwrap();
        let bytes = std::fs::read(installer_path).unwrap();
        update.verify(&bytes).unwrap();
        let mut modified = bytes.clone();
        modified[0] ^= 1;
        assert!(update.verify(&modified).is_err());
        println!(
            "Verified production updater signature/version {} ({} bytes); modified bytes rejected.",
            update.version,
            bytes.len()
        );
    }
}
