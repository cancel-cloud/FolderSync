use foldersync_engine::{watch::WatchService, *};
use std::{fs, time::Instant};
fn main() -> anyhow::Result<()> {
    let root = std::env::temp_dir().join(format!("FolderSync-benchmark-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(root.join("A"))?;
    fs::create_dir(root.join("B"))?;
    let t = Task {
        a: root.join("A"),
        b: root.join("B"),
        ..Task::default()
    };
    let data = vec![13u8; 1024];
    for i in 0..10000 {
        let name = format!("folder{:03}/file{:05}.dat", i / 100, i);
        fs::create_dir_all(t.a.join(&name).parent().unwrap())?;
        fs::write(t.a.join(&name), &data)?;
        fs::create_dir_all(t.b.join(&name).parent().unwrap())?;
        fs::write(t.b.join(name), &data)?;
    }
    let mut e = Engine::open(&root.join("state"))?;
    e.save_task(&t)?;
    let start = Instant::now();
    let p = e.preview(&t.id)?;
    let initial_ms = start.elapsed().as_millis();
    let initial = p.metrics.clone();
    e.apply(&p.id, true)?;
    let baseline_apply_start = Instant::now();
    // The baseline above was already applied; the next phase measures events.
    let watch = WatchService::new(&t)?;
    let changed = "folder050/file05000.dat";
    let event_start = Instant::now();
    fs::write(t.a.join(changed), vec![27; 1024])?;
    let mut event_count = 0;
    loop {
        let batch = watch.poll();
        event_count += batch.events;
        if batch.paths.iter().any(|p| p == changed) {
            e.queue_paths(&t.id, &[changed.into()], batch.uncertain)?;
            break;
        }
        anyhow::ensure!(
            event_start.elapsed() < std::time::Duration::from_secs(10),
            "No native benchmark event"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let start = Instant::now();
    let p = e.preview_paths(&t.id, &[changed.into()])?;
    let incremental_ms = start.elapsed().as_millis();
    let incremental = p.metrics.clone();
    e.apply(&p.id, false)?;
    let event_apply_ms = event_start.elapsed().as_millis();
    let start = Instant::now();
    let p = e.preview(&t.id)?;
    let full_ms = start.elapsed().as_millis();
    let _ = baseline_apply_start;
    let report = serde_json::json!({"files_per_side":10000,"bytes_per_file":1024,"fixture":root,"initial_discovery":{"wall_ms":initial_ms,"metrics":initial},"incremental_discovery":{"wall_ms":incremental_ms,"metrics":incremental},"native_event_through_apply_ms":event_apply_ms,"native_events":event_count,"forced_full_discovery":{"wall_ms":full_ms,"metrics":p.metrics},"peak_memory":"Measured separately by process PeakWorkingSet64"});
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
