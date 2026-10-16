//! Prepare only the explicitly owned GUI acceptance task for recovery QA.
//! Run with FolderSync stopped; no synchronized files are changed.
use anyhow::{Result, ensure};
use foldersync_engine::Engine;
use std::path::PathBuf;
fn main() -> Result<()> {
    let storage = PathBuf::from(std::env::args().nth(1).expect("app-data directory"));
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let fixture = workspace.join(".tools/FolderSync-AutoDelete-GUI");
    let mut engine = Engine::open(&storage)?;
    let id = "d849e90f-ec00-4b67-93b5-c09bfc8dca1f";
    let task = engine
        .tasks()?
        .into_iter()
        .find(|t| t.id == id)
        .expect("GUI fixture task required");
    ensure!(
        task.name == "FolderSync automatic deletion acceptance",
        "Fixture name mismatch"
    );
    ensure!(
        task.a.canonicalize()? == fixture.join("A").canonicalize()?
            && task.b.canonicalize()? == fixture.join("B").canonicalize()?,
        "Fixture roots mismatch"
    );
    ensure!(
        !task.monitor
            && !task.login
            && !task.deletion_policy.enabled
            && (task.schedule.kind.is_empty() || task.schedule.kind == "manual"),
        "Fixture automation must be off"
    );
    ensure!(
        !engine.versions(id)?.is_empty(),
        "Protected fixture version required"
    );
    engine.delete_task(id)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&engine.recovery_tasks()?)?
    );
    Ok(())
}
