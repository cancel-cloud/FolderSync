use crate::*;
use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, sync_channel},
    },
};
pub struct WatchService {
    _watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<Event>>,
    roots: [PathBuf; 2],
    overflow: Arc<AtomicBool>,
}
#[derive(Default, Debug)]
pub struct EventBatch {
    pub paths: Vec<String>,
    pub uncertain: bool,
    pub events: u64,
}
impl WatchService {
    pub fn new(task: &Task) -> Result<Self> {
        let (tx, receiver) = sync_channel(2048);
        let overflow = Arc::new(AtomicBool::new(false));
        let flag = overflow.clone();
        let mut watcher = notify::recommended_watcher(move |event| {
            if tx.try_send(event).is_err() {
                flag.store(true, Ordering::Relaxed);
            }
        })?;
        watcher.watch(&task.a, RecursiveMode::Recursive)?;
        watcher.watch(&task.b, RecursiveMode::Recursive)?;
        Ok(Self {
            _watcher: watcher,
            receiver,
            roots: [task.a.clone(), task.b.clone()],
            overflow,
        })
    }
    pub fn poll(&self) -> EventBatch {
        let mut batch = EventBatch {
            uncertain: self.overflow.swap(false, Ordering::Relaxed),
            ..EventBatch::default()
        };
        let mut paths = BTreeSet::new();
        while let Ok(result) = self.receiver.try_recv() {
            match result {
                Err(_) => batch.uncertain = true,
                Ok(event) => {
                    if matches!(event.kind, EventKind::Access(_)) {
                        continue;
                    }
                    batch.events += 1;
                    if event.need_rescan() {
                        batch.uncertain = true;
                    }
                    for path in event.paths {
                        for root in &self.roots {
                            if let Ok(rel) = path.strip_prefix(root) {
                                if rel.as_os_str().is_empty() {
                                    batch.uncertain = true;
                                } else {
                                    let p = rel.to_string_lossy().replace('\\', "/");
                                    if !p.split('/').any(|p| p.starts_with(".foldersync-stage-")) {
                                        paths.insert(p);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        batch.paths = paths.into_iter().collect();
        batch
    }
}
