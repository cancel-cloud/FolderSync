//! Read the disposable 35k fixture; write only fresh, isolated benchmark outputs.
use anyhow::{Result, ensure};
use foldersync_engine::*;
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

fn main() -> Result<()> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let source = repo.join(".tools/FolderSync-35000-Test/Quelle");
    ensure!(source.is_dir(), "Dedicated fixture missing");
    let output = repo
        .join(".tools")
        .join(format!("FolderSync-Perf-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&output)?;
    let files: Vec<_> = walkdir::WalkDir::new(&source)
        .into_iter()
        .map(|e| e.map_err(anyhow::Error::from))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();
    let total: u64 = files
        .iter()
        .map(|p| fs::metadata(p).map(|m| m.len()))
        .collect::<std::io::Result<Vec<_>>>()?
        .iter()
        .sum();
    let mut report = json!({"fixture":source,"output":output,"files":files.len(),"bytes":total,"build":"release", "version":env!("CARGO_PKG_VERSION"),"raw_hash":[],"samples":[]});
    let report_path = repo.join("docs/product/benchmarks/profile-local-latest.json");
    let save = |r: &serde_json::Value| -> Result<()> {
        fs::write(&report_path, serde_json::to_string_pretty(r)?)?;
        Ok(())
    };
    let quick = std::env::args().any(|arg| arg == "--quick");
    let raw_workers: &[usize] = if quick {
        &[10, 1]
    } else {
        &[1, 2, 4, 10, 10, 4, 2, 1]
    };
    for &workers in raw_workers {
        let next = AtomicUsize::new(0);
        let start = Instant::now();
        let metrics = std::thread::scope(|s| -> Result<Vec<Metrics>> {
            let handles: Vec<_> = (0..workers)
                .map(|_| {
                    s.spawn(|| -> Result<Metrics> {
                        let mut m = Metrics::default();
                        loop {
                            let i = next.fetch_add(1, Ordering::Relaxed);
                            let Some(p) = files.get(i) else { break };
                            foldersync_engine::fs::hash_file(p, &mut m)?;
                        }
                        Ok(m)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        })?;
        let row = json!({"workers":workers,"wall_ms":start.elapsed().as_millis(),"hashed_bytes":metrics.iter().map(|m|m.hashed_bytes).sum::<u64>()});
        eprintln!("raw hash: {row}");
        report["raw_hash"].as_array_mut().unwrap().push(row);
        save(&report)?;
    }
    let start = Instant::now();
    let mut m = Metrics::default();
    let tree = foldersync_engine::fs::scan(&source, &Scope::default(), &mut m)?;
    report["safe_scan"] =
        json!({"wall_ms":start.elapsed().as_millis(),"metrics":m,"entries":tree.len()});
    eprintln!("safe scan: {}", report["safe_scan"]);
    save(&report)?;
    let dest = output.join("full-preview-destination");
    fs::create_dir(&dest)?;
    let mut engine = Engine::open(&output.join("full-preview-state"))?;
    let task = Task {
        a: source.clone(),
        b: dest,
        mode: Mode::TwoWay,
        ..Task::default()
    };
    engine.save_task(&task)?;
    let start = Instant::now();
    let p = engine.preview(&task.id)?;
    report["full_preview"] = json!({"wall_ms":start.elapsed().as_millis(),"metrics":p.metrics,"operations":p.operations.len()});
    eprintln!("full preview: {}", report["full_preview"]);
    save(&report)?;
    drop(engine);
    let folders: Vec<_> = tree
        .values()
        .filter(|e| e.directory && !e.path.contains('/'))
        .take(10)
        .map(|e| e.path.clone())
        .collect();
    let copy_workers: &[u32] = if quick { &[4] } else { &[1, 2, 4, 10] };
    for &workers in copy_workers {
        let dest = output.join(format!("copy-{workers}"));
        fs::create_dir(&dest)?;
        let mut e = Engine::open(&output.join(format!("state-{workers}")))?;
        let task = Task {
            a: source.clone(),
            b: dest.clone(),
            mode: Mode::Copy,
            transfer_concurrency: workers,
            scope: Scope {
                selected_folders: folders.clone(),
                ..Scope::default()
            },
            ..Task::default()
        };
        e.save_task(&task)?;
        let start = Instant::now();
        let p = e.preview(&task.id)?;
        let preview_ms = start.elapsed().as_millis();
        let progress = e.progress.clone();
        let done = std::sync::atomic::AtomicBool::new(false);
        let start = Instant::now();
        let (applied, phases) = std::thread::scope(|s| {
            let monitor = s.spawn(|| {
                let mut phases = std::collections::BTreeMap::<String, u128>::new();
                let mut last = Instant::now();
                let mut phase = String::from("Starting");
                while !done.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    let now = Instant::now();
                    *phases.entry(phase).or_default() += now.duration_since(last).as_millis();
                    last = now;
                    phase = progress
                        .snapshot()
                        .map(|p| format!("{:?}", p.phase))
                        .unwrap_or_else(|| "Idle".into());
                }
                phases
            });
            let p = e.apply(&p.id, false);
            done.store(true, Ordering::Relaxed);
            (p, monitor.join().unwrap())
        });
        let apply_ms = start.elapsed().as_millis();
        let applied = applied?;
        ensure!(applied.status == "Applied", "Sample failed");
        let mut actual = Metrics::default();
        let verify = foldersync_engine::fs::scan(&dest, &Scope::default(), &mut actual)?;
        for o in &applied.operations {
            if let Some(a) = &o.a {
                ensure!(
                    verify
                        .get(&foldersync_engine::fs::path_key(&o.path))
                        .map(|e| &e.digest)
                        == Some(&a.digest),
                    "Copied content mismatch"
                );
            }
        }
        let row = json!({"workers":workers,"preview_ms":preview_ms,"apply_ms":apply_ms,"phase_samples_ms":phases,"metrics":applied.metrics,"verified_files":verify.values().filter(|e|!e.directory).count()});
        eprintln!("sample: {row}");
        report["samples"].as_array_mut().unwrap().push(row);
        save(&report)?;
    }
    let db = rusqlite::Connection::open(output.join("flush-cost.sqlite3"))?;
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE test(id INTEGER PRIMARY KEY,value TEXT)")?;
    let start = Instant::now();
    for i in 0..1000 {
        db.execute(
            "INSERT INTO test VALUES(?,?)",
            rusqlite::params![i, "flush"],
        )?;
    }
    let individual = start.elapsed().as_millis();
    let start = Instant::now();
    db.execute_batch("BEGIN")?;
    for i in 1000..2000 {
        db.execute(
            "INSERT INTO test VALUES(?,?)",
            rusqlite::params![i, "flush"],
        )?;
    }
    db.execute_batch("COMMIT")?;
    report["durable_sqlite"] =
        json!({"individual_1000_ms":individual,"transaction_1000_ms":start.elapsed().as_millis()});
    save(&report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
