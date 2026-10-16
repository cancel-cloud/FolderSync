use crate::*;
use anyhow::Result;
use notify::event::{ModifyKind, RenameMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, sync_channel},
    },
};
pub struct WatchService {
    _watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<Event>>,
    roots: [PathBuf; 2],
    overflow: Arc<AtomicBool>,
    pending_renames: Mutex<[Option<(String, std::time::Instant)>; 2]>,
}
#[derive(Default, Debug)]
pub struct EventBatch {
    pub paths: Vec<String>,
    pub uncertain: bool,
    pub events: u64,
    pub renames: Vec<RenameHint>,
}
#[derive(Debug)]
pub struct RenameHint {
    pub side: String,
    pub from: String,
    pub to: String,
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
            pending_renames: Mutex::new([None, None]),
        })
    }
    pub fn poll(&self) -> EventBatch {
        let mut batch = EventBatch {
            uncertain: self.overflow.swap(false, Ordering::Relaxed),
            ..EventBatch::default()
        };
        let mut paths = BTreeSet::new();
        let Ok(mut pending) = self.pending_renames.lock() else {
            batch.uncertain = true;
            return batch;
        };
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
                    if let EventKind::Modify(ModifyKind::Name(mode)) = event.kind {
                        for (side, root) in self.roots.iter().enumerate() {
                            let relative: Vec<_> = event
                                .paths
                                .iter()
                                .filter_map(|p| p.strip_prefix(root).ok())
                                .map(|p| p.to_string_lossy().replace('\\', "/"))
                                .collect();
                            match (mode, relative.as_slice()) {
                                (RenameMode::From, [from]) => {
                                    if pending[side].is_some() {
                                        batch.uncertain = true;
                                        // Empty name suppresses pairing until the
                                        // ambiguous sequence is drained.
                                        pending[side] =
                                            Some((String::new(), std::time::Instant::now()));
                                    } else {
                                        pending[side] =
                                            Some((from.clone(), std::time::Instant::now()));
                                    }
                                }
                                (RenameMode::To, [to]) => {
                                    if let Some((from, _)) = pending[side].take() {
                                        if from.is_empty() {
                                            batch.uncertain = true;
                                        } else if !internal(&from) && !internal(to) {
                                            batch.renames.push(RenameHint {
                                                side: if side == 0 { "A" } else { "B" }.into(),
                                                from,
                                                to: to.clone(),
                                            });
                                        }
                                    } else if !internal(to) {
                                        batch.uncertain = true;
                                    }
                                }
                                (RenameMode::Both, [from, to]) => {
                                    if !internal(from) && !internal(to) {
                                        batch.renames.push(RenameHint {
                                            side: if side == 0 { "A" } else { "B" }.into(),
                                            from: from.clone(),
                                            to: to.clone(),
                                        });
                                    }
                                }
                                (_, []) => {}
                                _ => batch.uncertain = true,
                            }
                        }
                    }
                    for path in event.paths {
                        for root in &self.roots {
                            if let Ok(rel) = path.strip_prefix(root) {
                                if rel.as_os_str().is_empty() {
                                    batch.uncertain = true;
                                } else {
                                    let p = rel.to_string_lossy().replace('\\', "/");
                                    if !internal(&p) {
                                        paths.insert(p);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        for rename in pending.iter_mut() {
            if rename
                .as_ref()
                .is_some_and(|(_, at)| at.elapsed() >= std::time::Duration::from_secs(1))
            {
                *rename = None;
                batch.uncertain = true;
            }
        }
        batch.paths = paths.into_iter().collect();
        batch
    }
}
fn internal(path: &str) -> bool {
    path.split('/')
        .any(|p| p.starts_with(".foldersync-stage-") || p.starts_with(".foldersync-displaced-"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn injected_events() -> (
        tempfile::TempDir,
        std::sync::mpsc::SyncSender<notify::Result<Event>>,
        WatchService,
    ) {
        let d = tempfile::Builder::new()
            .prefix("FolderSync-rename-events-")
            .tempdir()
            .unwrap();
        let t = Task {
            a: d.path().join("A"),
            b: d.path().join("B"),
            ..Task::default()
        };
        std::fs::create_dir(&t.a).unwrap();
        std::fs::create_dir(&t.b).unwrap();
        let mut w = WatchService::new(&t).unwrap();
        let (tx, rx) = sync_channel(16);
        w.receiver = rx;
        (d, tx, w)
    }
    fn rename(mode: RenameMode, path: PathBuf) -> notify::Result<Event> {
        Ok(Event::new(EventKind::Modify(ModifyKind::Name(mode))).add_path(path))
    }
    #[test]
    fn split_rename_pair_is_preserved_but_internal_staging_never_becomes_a_candidate() {
        let (_d, tx, w) = injected_events();
        tx.send(rename(RenameMode::From, w.roots[0].join("old")))
            .unwrap();
        let first = w.poll();
        assert_eq!(first.paths, vec!["old"]);
        assert!(first.renames.is_empty() && !first.uncertain);
        tx.send(rename(RenameMode::To, w.roots[0].join("new")))
            .unwrap();
        let second = w.poll();
        assert_eq!(second.renames.len(), 1);
        assert_eq!(second.renames[0].from, "old");
        assert_eq!(second.renames[0].to, "new");
        assert_eq!(second.renames[0].side, "A");
        assert!(!second.uncertain);
        tx.send(rename(
            RenameMode::From,
            w.roots[1].join(".foldersync-stage-owned"),
        ))
        .unwrap();
        tx.send(rename(RenameMode::To, w.roots[1].join("verified")))
            .unwrap();
        let own = w.poll();
        assert!(own.renames.is_empty() && !own.uncertain);
        assert_eq!(own.paths, vec!["verified"]);
        w.pending_renames.lock().unwrap()[0] = Some((
            "orphan".into(),
            std::time::Instant::now() - std::time::Duration::from_secs(2),
        ));
        assert!(w.poll().uncertain);
    }
    #[test]
    fn multiple_old_names_before_a_new_name_request_reconciliation_without_guessing_a_pair() {
        let (_d, tx, w) = injected_events();
        tx.send(rename(RenameMode::From, w.roots[0].join("old0")))
            .unwrap();
        tx.send(rename(RenameMode::From, w.roots[0].join("old1")))
            .unwrap();
        tx.send(rename(RenameMode::To, w.roots[0].join("new")))
            .unwrap();
        let batch = w.poll();
        assert!(batch.uncertain);
        assert!(batch.renames.is_empty());
    }
    #[test]
    fn injected_overflow_marks_uncertainty_instead_of_dropping_it_silently() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("A");
        let b = d.path().join("B");
        std::fs::create_dir(&a).unwrap();
        std::fs::create_dir(&b).unwrap();
        let t = Task {
            a,
            b,
            ..Task::default()
        };
        let watch = WatchService::new(&t).unwrap();
        watch.overflow.store(true, Ordering::Relaxed);
        assert!(watch.poll().uncertain);
        assert!(!watch.poll().uncertain);
    }
}
