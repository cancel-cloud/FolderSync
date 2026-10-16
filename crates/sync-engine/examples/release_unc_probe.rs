//! Read-only rejection probe when the dedicated release QA share is unavailable.
//! Does not create a share, authenticate, or touch any other network path.
use anyhow::{Context, Result, ensure};
use foldersync_engine::{Engine, Task};
use std::{fs, path::PathBuf};

fn main() -> Result<()> {
    let unc = std::env::args()
        .nth(1)
        .context("Pass the dedicated QA UNC path")?;
    ensure!(
        unc.starts_with(r"\\") && unc.split('\\').nth(3) == Some("FolderSyncReleaseQA"),
        "Only the dedicated FolderSyncReleaseQA share is permitted"
    );
    let d = tempfile::Builder::new()
        .prefix("FolderSync-UNC-release-")
        .tempdir()?;
    let a = d.path().join("A");
    fs::create_dir(&a)?;
    fs::write(
        a.join("keep.txt"),
        b"FolderSync disposable UNC safety sentinel",
    )?;
    let mut e = Engine::open(&d.path().join("state"))?;
    let t = Task {
        name: "Disposable UNC release validation".into(),
        a: a.clone(),
        b: PathBuf::from(&unc),
        ..Task::default()
    };
    let error = e
        .save_task(&t)
        .expect_err("Unavailable QA UNC must not be accepted");
    let detail = format!("{error:#}");
    ensure!(
        detail.contains("Root unavailable"),
        "Expected root availability rejection: {detail}"
    );
    ensure!(e.tasks()?.is_empty(), "Rejected pair must not persist");
    ensure!(
        fs::read(a.join("keep.txt"))? == b"FolderSync disposable UNC safety sentinel",
        "Local source changed"
    );
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"unc":unc,"result":"unavailable UNC rejected","error":detail,"local_source_preserved":true,"tasks_persisted":0,"real_smb_transfer_and_reconnection":"not verified: standard-user share creation denied"})
        )?
    );
    Ok(())
}
