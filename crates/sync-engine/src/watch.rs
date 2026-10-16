use crate::*;
use anyhow::Result;
use notify::event::{ModifyKind, RenameMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const MAX_PATHS: usize = 65_536;
const MAX_BUFFERED_BYTES: usize = 16 * 1024 * 1024;

pub struct WatchService {
    _watcher: RecommendedWatcher,
    collector: Arc<Mutex<Collector>>,
    roots: [PathBuf; 2],
    identities: [String; 2],
    health_checked: Mutex<Instant>,
}
#[derive(Default, Debug)]
pub struct EventBatch {
    pub paths: Vec<String>,
    pub uncertain: bool,
    pub watch_lost: bool,
    pub events: u64,
    pub renames: Vec<RenameHint>,
}
#[derive(Debug)]
pub struct RenameHint {
    pub side: String,
    pub from: String,
    pub to: String,
}

// The native watcher invokes this collector independently of the engine.
// Store unique dirty paths rather than raw save/write events while it is busy.
struct Collector {
    roots: [PathBuf; 2],
    paths: BTreeSet<String>,
    batch: EventBatch,
    pending_renames: [Option<(String, Instant)>; 2],
    buffered_bytes: usize,
}
impl Collector {
    fn new(roots: [PathBuf; 2]) -> Self {
        Self {
            roots,
            paths: BTreeSet::new(),
            batch: EventBatch::default(),
            pending_renames: [None, None],
            buffered_bytes: 0,
        }
    }
    fn invalidate(&mut self) {
        self.batch.uncertain = true;
        self.paths.clear();
        self.batch.renames.clear();
        self.pending_renames = [None, None];
        self.buffered_bytes = 0;
    }
    fn insert_path(&mut self, path: String) {
        if self.paths.contains(&path) {
            return;
        }
        if self.paths.len() >= MAX_PATHS
            || self.buffered_bytes.saturating_add(path.len()) > MAX_BUFFERED_BYTES
        {
            self.invalidate();
            return;
        }
        self.buffered_bytes += path.len();
        self.paths.insert(path);
    }
    fn insert_rename(&mut self, hint: RenameHint) {
        let bytes = hint.from.len().saturating_add(hint.to.len());
        if self.batch.renames.len() >= MAX_PATHS
            || self.buffered_bytes.saturating_add(bytes) > MAX_BUFFERED_BYTES
        {
            self.invalidate();
            return;
        }
        self.buffered_bytes += bytes;
        self.batch.renames.push(hint);
    }
    fn expire_renames(&mut self) {
        for rename in &mut self.pending_renames {
            if rename
                .as_ref()
                .is_some_and(|(_, at)| at.elapsed() >= Duration::from_secs(1))
            {
                *rename = None;
                self.batch.uncertain = true;
            }
        }
    }
    fn receive(&mut self, result: notify::Result<Event>) {
        self.expire_renames();
        match result {
            Err(_) => {
                self.batch.watch_lost = true;
                self.invalidate();
            }
            Ok(event) => {
                if matches!(event.kind, EventKind::Access(_)) {
                    return;
                }
                self.batch.events = self.batch.events.saturating_add(1);
                if self.batch.uncertain {
                    return;
                }
                if event.need_rescan() {
                    self.batch.uncertain = true;
                }
                if let EventKind::Modify(ModifyKind::Name(mode)) = event.kind {
                    for (side, root) in self.roots.clone().iter().enumerate() {
                        let relative: Vec<_> = event
                            .paths
                            .iter()
                            .filter_map(|p| p.strip_prefix(root).ok())
                            .map(|p| p.to_string_lossy().replace('\\', "/"))
                            .collect();
                        match (mode, relative.as_slice()) {
                            (RenameMode::From, [from]) => {
                                if self.pending_renames[side].is_some() {
                                    self.batch.uncertain = true;
                                    // Empty name suppresses pairing until the
                                    // ambiguous sequence is drained.
                                    self.pending_renames[side] =
                                        Some((String::new(), std::time::Instant::now()));
                                } else {
                                    self.pending_renames[side] =
                                        Some((from.clone(), std::time::Instant::now()));
                                }
                            }
                            (RenameMode::To, [to]) => {
                                if let Some((from, _)) = self.pending_renames[side].take() {
                                    if from.is_empty() {
                                        self.batch.uncertain = true;
                                    } else if !internal(&from) && !internal(to) {
                                        self.insert_rename(RenameHint {
                                            side: if side == 0 { "A" } else { "B" }.into(),
                                            from,
                                            to: to.clone(),
                                        });
                                    }
                                } else if !internal(to) {
                                    self.batch.uncertain = true;
                                }
                            }
                            (RenameMode::Both, [from, to]) => {
                                if !internal(from) && !internal(to) {
                                    self.insert_rename(RenameHint {
                                        side: if side == 0 { "A" } else { "B" }.into(),
                                        from: from.clone(),
                                        to: to.clone(),
                                    });
                                }
                            }
                            (_, []) => {}
                            _ => self.batch.uncertain = true,
                        }
                    }
                }
                for path in event.paths {
                    for root in &self.roots.clone() {
                        if let Ok(rel) = path.strip_prefix(root) {
                            if rel.as_os_str().is_empty() {
                                self.batch.uncertain = true;
                            } else {
                                let p = rel.to_string_lossy().replace('\\', "/");
                                if !internal(&p) {
                                    self.insert_path(p);
                                }
                            }
                        }
                    }
                }
            }
        }
        if self.batch.uncertain {
            // A complete reconciliation supersedes individual paths and hints.
            self.invalidate();
        }
    }
    fn drain(&mut self) -> EventBatch {
        self.expire_renames();
        let mut batch = std::mem::take(&mut self.batch);
        batch.paths = std::mem::take(&mut self.paths).into_iter().collect();
        self.buffered_bytes = 0;
        batch
    }
}
impl WatchService {
    pub fn new(task: &Task) -> Result<Self> {
        let roots = [task.a.clone(), task.b.clone()];
        let identities = [
            crate::fs::root_identity(&task.a)?,
            crate::fs::root_identity(&task.b)?,
        ];
        let collector = Arc::new(Mutex::new(Collector::new([task.a.clone(), task.b.clone()])));
        let sink = collector.clone();
        let mut watcher = notify::recommended_watcher(move |event| match sink.lock() {
            Ok(mut collector) => collector.receive(event),
            Err(poisoned) => poisoned.into_inner().invalidate(),
        })?;
        watcher.watch(&task.a, RecursiveMode::Recursive)?;
        watcher.watch(&task.b, RecursiveMode::Recursive)?;
        anyhow::ensure!(
            identities
                == [
                    crate::fs::root_identity(&task.a)?,
                    crate::fs::root_identity(&task.b)?
                ],
            "Roots changed while establishing monitoring"
        );
        Ok(Self {
            _watcher: watcher,
            collector,
            roots,
            identities,
            health_checked: Mutex::new(Instant::now()),
        })
    }
    pub fn poll(&self) -> EventBatch {
        // Windows does not report all changes to the watched directory itself.
        // Probe only the roots, never enumerate the full trees for this check.
        let unhealthy = match self.health_checked.lock() {
            Ok(mut checked) if checked.elapsed() >= Duration::from_secs(30) => {
                *checked = Instant::now();
                self.roots
                    .iter()
                    .zip(&self.identities)
                    .any(|(root, identity)| {
                        crate::fs::root_identity(root).as_ref().ok() != Some(identity)
                    })
            }
            Ok(_) => false,
            Err(_) => true,
        };
        if unhealthy {
            let mut collector = self.collector.lock().unwrap_or_else(|p| p.into_inner());
            collector.batch.watch_lost = true;
            collector.invalidate();
        }
        match self.collector.lock() {
            Ok(mut collector) => collector.drain(),
            Err(poisoned) => {
                let mut collector = poisoned.into_inner();
                collector.invalidate();
                collector.drain()
            }
        }
    }
    /// A failed durable enqueue must trigger recovery on the next attempt.
    pub fn invalidate(&self) {
        match self.collector.lock() {
            Ok(mut collector) => collector.invalidate(),
            Err(poisoned) => poisoned.into_inner().invalidate(),
        }
    }
}
fn internal(path: &str) -> bool {
    path.split('/')
        .any(|p| p.starts_with(".foldersync-stage-") || p.starts_with(".foldersync-displaced-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirty_five_thousand_changes_fit_without_polling_and_duplicate_saves_coalesce() {
        let (_dir, tx, watch) = injected_events();
        for i in 0..35_350 {
            let path = watch.roots[0].join(format!("group/file-{i}.bin"));
            for _ in 0..3 {
                tx.send(Ok(Event::new(EventKind::Any).add_path(path.clone())))
                    .unwrap();
            }
        }
        let batch = watch.poll();
        assert!(!batch.uncertain);
        assert_eq!(batch.paths.len(), 35_350);
        assert_eq!(batch.events, 106_050);
        assert!(watch.poll().paths.is_empty());
    }

    #[test]
    fn collector_limits_and_backend_errors_require_reconciliation() {
        let (_dir, tx, watch) = injected_events();
        for i in 0..=MAX_PATHS {
            tx.send(Ok(
                Event::new(EventKind::Any).add_path(watch.roots[0].join(format!("f{i}")))
            ))
            .unwrap();
        }
        let overflow = watch.poll();
        assert!(overflow.uncertain && overflow.paths.is_empty());
        assert!(!watch.poll().uncertain);
        tx.send(Err(notify::Error::generic("watch disconnected")))
            .unwrap();
        let lost = watch.poll();
        assert!(lost.uncertain && lost.watch_lost);
        tx.send(Ok(
            Event::new(EventKind::Any).set_flag(notify::event::Flag::Rescan)
        ))
        .unwrap();
        assert!(watch.poll().uncertain);
    }

    #[test]
    fn byte_limit_and_rename_hint_limit_are_bounded() {
        let mut collector = Collector::new([PathBuf::from("A"), PathBuf::from("B")]);
        collector.insert_path("x".repeat(MAX_BUFFERED_BYTES + 1));
        assert!(collector.drain().uncertain);
        for i in 0..=MAX_PATHS {
            collector.insert_rename(RenameHint {
                side: "A".into(),
                from: i.to_string(),
                to: "new".into(),
            });
        }
        assert!(collector.drain().uncertain);
    }

    #[test]
    fn root_unavailable_is_watch_loss_instead_of_a_deletion_signal() {
        let dir = tempfile::tempdir().unwrap();
        let task = Task {
            a: dir.path().join("A"),
            b: dir.path().join("B"),
            ..Task::default()
        };
        std::fs::create_dir(&task.a).unwrap();
        std::fs::create_dir(&task.b).unwrap();
        let watch = WatchService::new(&task).unwrap();
        std::fs::rename(&task.a, dir.path().join("old-A")).unwrap();
        *watch.health_checked.lock().unwrap() = Instant::now() - Duration::from_secs(31);
        let batch = watch.poll();
        assert!(batch.uncertain && batch.watch_lost);
        assert!(batch.paths.is_empty());
        #[cfg(windows)]
        {
            std::fs::create_dir(&task.a).unwrap();
            *watch.health_checked.lock().unwrap() = Instant::now() - Duration::from_secs(31);
            let replaced = watch.poll();
            assert!(
                replaced.uncertain && replaced.watch_lost,
                "Same path with a different directory identity must not resume the old watch"
            );
        }
    }

    #[test]
    fn concurrent_drains_preserve_every_received_event() {
        let (_dir, tx, watch) = injected_events();
        std::thread::scope(|threads| {
            let producer = threads.spawn(|| {
                for index in 0..10_000 {
                    tx.send(Ok(Event::new(EventKind::Any)
                        .add_path(watch.roots[0].join(format!("f{}", index % 10)))))
                        .unwrap();
                }
            });
            let mut events = 0;
            while !producer.is_finished() {
                let batch = watch.poll();
                assert!(!batch.uncertain);
                events += batch.events;
                std::thread::yield_now();
            }
            producer.join().unwrap();
            events += watch.poll().events;
            assert_eq!(events, 10_000);
        });
    }
    struct Injection(Arc<Mutex<Collector>>);
    impl Injection {
        fn send(&self, event: notify::Result<Event>) -> Result<()> {
            self.0.lock().unwrap().receive(event);
            Ok(())
        }
    }
    struct TestWatch {
        roots: [PathBuf; 2],
        collector: Arc<Mutex<Collector>>,
    }
    impl TestWatch {
        fn poll(&self) -> EventBatch {
            self.collector.lock().unwrap().drain()
        }
    }
    fn injected_events() -> (tempfile::TempDir, Injection, TestWatch) {
        let d = tempfile::Builder::new()
            .prefix("FolderSync-rename-events-")
            .tempdir()
            .unwrap();
        let roots = [d.path().join("A"), d.path().join("B")];
        let collector = Arc::new(Mutex::new(Collector::new(roots.clone())));
        (
            d,
            Injection(collector.clone()),
            TestWatch { roots, collector },
        )
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
        w.collector.lock().unwrap().pending_renames[0] = Some((
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
        watch.invalidate();
        assert!(watch.poll().uncertain);
        assert!(!watch.poll().uncertain);
    }
}
