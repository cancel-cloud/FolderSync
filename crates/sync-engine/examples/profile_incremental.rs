//! Disposable benchmark: no existing folders, tasks or application DB are used.
use anyhow::{Result, ensure};
use foldersync_engine::{watch::WatchService, *};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() -> Result<()> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let reuse = std::env::args().nth(1);
    let output = if let Some(path) = &reuse {
        let path = fs::canonicalize(path)?;
        ensure!(
            path.starts_with(fs::canonicalize(repo.join(".tools"))?)
                && path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("FolderSync-Incremental-"),
            "Reuse only this harness's disposable .tools fixture"
        );
        ensure!(
            fs::canonicalize(path.join("A"))?.starts_with(&path)
                && fs::canonicalize(path.join("B"))?.starts_with(&path),
            "Fixture roots must remain inside the disposable output"
        );
        path
    } else {
        let path = repo
            .join(".tools")
            .join(format!("FolderSync-Incremental-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path)?;
        path
    };
    let task = Task {
        a: output.join("A"),
        b: output.join("B"),
        ..Task::default()
    };
    if reuse.is_none() {
        fs::create_dir(&task.a)?;
        fs::create_dir(&task.b)?;
    }
    let count = 35_000usize;
    let mut bytes = 0u64;
    for index in 0..count {
        let relative = format!("group-{:03}/file-{index:05}.bin", index / 100);
        let content = vec![(index % 251) as u8; [128, 1024, 4096, 32768][index % 4]];
        for root in [&task.a, &task.b] {
            let path = root.join(&relative);
            if reuse.is_none() {
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(path, &content)?;
            } else {
                ensure!(fs::read(path)? == content, "Reused fixture content changed");
            }
        }
        bytes += content.len() as u64;
    }
    eprintln!(
        "Generated {count} files per side ({bytes} bytes per side) at {}",
        output.display()
    );
    let mut engine = Engine::open(&output.join(format!("state-{}", uuid::Uuid::new_v4())))?;
    engine.save_task(&task)?;
    let start = Instant::now();
    let preview = engine.preview(&task.id)?;
    let manual = engine.apply(&preview.id, false)?;
    let manual_ms = start.elapsed().as_millis();
    ensure!(
        manual
            .operations
            .iter()
            .all(|o| o.action == Action::Unchanged)
    );
    let start = Instant::now();
    engine.queue_paths(&task.id, &[], true)?;
    let automatic = engine.process_queue(&task.id)?.unwrap();
    let automatic_ms = start.elapsed().as_millis();
    ensure!(
        automatic
            .operations
            .iter()
            .all(|o| o.action == Action::Unchanged)
    );
    ensure!(automatic.metrics.hashed_bytes == bytes * 2);
    let watch = WatchService::new(&task)?;
    let relative = "group-000/file-00000.bin";
    fs::write(task.a.join(relative), b"a real incremental change")?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut observed = false;
    while Instant::now() < deadline {
        let batch = watch.poll();
        ensure!(!batch.uncertain, "Unexpected native watcher uncertainty");
        if batch.paths.iter().any(|p| p == relative) {
            engine.queue_watch_batch(&task.id, &batch)?;
            observed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    ensure!(observed, "Missing native change notification");
    let start = Instant::now();
    let incremental = engine.process_queue(&task.id)?.unwrap();
    let incremental_ms = start.elapsed().as_millis();
    ensure!(incremental.metrics.reconciliations == 0);
    ensure!(incremental.operations.len() < 10);
    ensure!(fs::read(task.b.join(relative))? == b"a real incremental change");
    let report = json!({
        "version": env!("CARGO_PKG_VERSION"), "build": "release", "fixture": output,
        "files_per_side": count, "bytes_per_side": bytes,
        "manual_preview_apply": {"wall_ms": manual_ms, "metrics": manual.metrics},
        "automatic_equal_reconciliation": {"wall_ms": automatic_ms, "metrics": automatic.metrics},
        "native_single_file_apply_excluding_debounce": {"wall_ms": incremental_ms, "metrics": incremental.metrics, "operations": incremental.operations.len()},
        "verified_single_file_destination": true
    });
    let path = repo.join("docs/product/benchmarks/incremental-0.1.9.json");
    fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
