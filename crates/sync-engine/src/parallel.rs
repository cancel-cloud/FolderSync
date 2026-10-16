use crate::{index::Index, *};
use anyhow::{Result, anyhow};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

fn file_transfer(o: &Operation) -> bool {
    matches!(
        o.action,
        Action::CopyToA | Action::CopyToB | Action::MoveToB
    ) && !o.a.iter().chain(o.b.iter()).any(|e| e.directory)
}
fn file_phase(o: &Operation) -> bool {
    !matches!(o.action, Action::DeleteA | Action::DeleteB)
        && !o.a.iter().chain(o.b.iter()).any(|e| e.directory)
}
enum Event {
    Completed(Box<(usize, Operation, Metrics, Result<()>)>),
    Failed(anyhow::Error),
}
impl Engine {
    /// Revalidate every reviewed input before the first filesystem mutation.
    /// Workers only read files; SQLite remains on the coordinating thread.
    pub(crate) fn validate_inputs(
        &self,
        task: &Task,
        operations: &[Operation],
        metrics: &mut Metrics,
    ) -> Result<()> {
        let next = AtomicUsize::new(0);
        let completed = AtomicUsize::new(0);
        let failed = std::sync::atomic::AtomicBool::new(false);
        let cancel = &self.cancel;
        let progress = &self.progress;
        let results = std::thread::scope(|threads| -> Result<Vec<(Metrics, Result<()>)>> {
            let mut handles = Vec::new();
            for _ in 0..crate::fs::comparison_workers().min(operations.len()) {
                let next = &next;
                let completed = &completed;
                let failed = &failed;
                handles.push(threads.spawn(move || -> (Metrics, Result<()>) {
                    let mut local = Metrics::default();
                    let result = (|| -> Result<()> {
                        let matcher = crate::fs::Matcher::new(&task.scope)?;
                        loop {
                            crate::progress::check_cancel(Some(cancel))?;
                            if failed.load(Ordering::Relaxed) {
                                break;
                            }
                            let i = next.fetch_add(1, Ordering::Relaxed);
                            let Some(operation) = operations.get(i) else {
                                break;
                            };
                            for (root, expected) in
                                [(&task.a, &operation.a), (&task.b, &operation.b)]
                            {
                                let result = crate::fs::read_entry_matched(
                                    root,
                                    &operation.path,
                                    &matcher,
                                    &mut local,
                                    Some(cancel),
                                    Some(progress),
                                );
                                if result.is_err() {
                                    failed.store(true, Ordering::Relaxed);
                                }
                                if &result? != expected {
                                    failed.store(true, Ordering::Relaxed);
                                    return Err(anyhow!(
                                        "Stale input: {}; preview again",
                                        operation.path
                                    ));
                                }
                            }
                            let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                            progress.update(|s| {
                                s.completed_entries = s.completed_entries.max(done as u64)
                            });
                        }
                        Ok(())
                    })();
                    (local, result)
                }));
            }
            handles
                .into_iter()
                .map(|h| h.join().map_err(|_| anyhow!("Validation worker panicked")))
                .collect()
        })?;
        let mut failure = None;
        for (local, result) in results {
            if let Err(error) = result {
                failure.get_or_insert(error);
            }
            metrics.scanned_entries += local.scanned_entries;
            metrics.hashed_bytes += local.hashed_bytes;
        }
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn execute_operations(
        &mut self,
        id: &str,
        task: &Task,
        operations: &mut [Operation],
        metrics: &mut Metrics,
    ) -> Result<()> {
        let mut position = 0;
        while position < operations.len() {
            if task.transfer_concurrency > 1 && file_transfer(&operations[position]) {
                let end = position
                    + operations[position..]
                        .iter()
                        .take_while(|o| file_phase(o))
                        .count();
                self.parallel_files(id, task, &mut operations[position..end], metrics, position)?;
                position = end;
            } else {
                self.apply_operation(id, task, &mut operations[position], metrics, position)?;
                position += 1;
            }
        }
        Ok(())
    }
    fn parallel_files(
        &mut self,
        id: &str,
        task: &Task,
        operations: &mut [Operation],
        metrics: &mut Metrics,
        before: usize,
    ) -> Result<()> {
        let mut jobs = vec![];
        let mut done = before;
        for (i, o) in operations.iter_mut().enumerate() {
            if file_transfer(o) {
                jobs.push((i, o.clone()));
            } else {
                self.apply_operation(id, task, o, metrics, done)?;
                done += 1;
            }
        }
        let budget_limit = if task.bandwidth_kib == 0 {
            16
        } else {
            task.bandwidth_kib.min(16) as usize
        };
        let count = (task.transfer_concurrency as usize)
            .min(jobs.len())
            .min(budget_limit);
        if count <= 1 {
            for (i, _) in jobs {
                self.apply_operation(id, task, &mut operations[i], metrics, done)?;
                done += 1;
            }
            return Ok(());
        }
        let mut worker_task = task.clone();
        if worker_task.bandwidth_kib > 0 {
            worker_task.bandwidth_kib /= count as u64;
        }
        let storage = self.storage().to_path_buf();
        let cancel = self.cancel.clone();
        let progress: Vec<_> = (0..count).map(|_| ProgressState::default()).collect();
        let next = AtomicUsize::new(0);
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let (sender, receiver) = mpsc::sync_channel(count);
        let mut failure = None;
        std::thread::scope(|scope| {
            let mut handles = vec![];
            for progress in &progress {
                let sender = sender.clone();
                let cancel = cancel.clone();
                let storage = &storage;
                let jobs = &jobs;
                let task = &worker_task;
                let next = &next;
                let active = &active;
                let peak = &peak;
                handles.push(scope.spawn(move || {
                    let index = match Index::worker(storage) {
                        Ok(index) => index,
                        Err(error) => {
                            let _ = sender.send(Event::Failed(error));
                            return;
                        }
                    };
                    let mut engine = Engine {
                        index,
                        cancel: cancel.clone(),
                        progress: progress.clone(),
                    };
                    while !cancel.load(Ordering::Relaxed) {
                        let position = next.fetch_add(1, Ordering::Relaxed);
                        let Some((i, original)) = jobs.get(position) else {
                            break;
                        };
                        let mut operation = original.clone();
                        let mut m = Metrics::default();
                        let guard = progress.begin(&task.id, Some(id), ProgressPhase::Applying, 1);
                        let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(current, Ordering::SeqCst);
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            engine.apply_operation(id, task, &mut operation, &mut m, 0)
                        }))
                        .unwrap_or_else(|_| Err(anyhow!("Transfer worker panicked")));
                        active.fetch_sub(1, Ordering::SeqCst);
                        drop(guard);
                        let failed = result.is_err();
                        if sender
                            .send(Event::Completed(Box::new((*i, operation, m, result))))
                            .is_err()
                        {
                            break;
                        }
                        if failed {
                            break;
                        }
                    }
                }));
            }
            drop(sender);
            loop {
                match receiver.recv_timeout(Duration::from_millis(50)) {
                    Ok(Event::Completed(completed)) => {
                        let (i, mut operation, m, result) = *completed;
                        metrics.scanned_entries += m.scanned_entries;
                        metrics.hashed_bytes += m.hashed_bytes;
                        metrics.copied_bytes += m.copied_bytes;
                        if let Err(error) = result {
                            operation.status = if error.is::<Cancelled>() {
                                "Cancelled"
                            } else {
                                "Failed"
                            }
                            .into();
                            if failure.is_none() {
                                failure = Some(error);
                            }
                            cancel.store(true, Ordering::Relaxed);
                        }
                        operations[i] = operation;
                        done += 1;
                    }
                    Ok(Event::Failed(error)) => {
                        if failure.is_none() {
                            failure = Some(error);
                        }
                        cancel.store(true, Ordering::Relaxed);
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                let snapshot = progress.iter().filter_map(ProgressState::snapshot).next();
                self.progress.update(|s| {
                    s.completed_entries = done as u64;
                    s.active_transfers = active.load(Ordering::SeqCst) as u32;
                    s.max_transfers = count as u32;
                    if let Some(snapshot) = snapshot {
                        s.phase = snapshot.phase;
                        s.path = snapshot.path;
                        s.file_bytes = snapshot.file_bytes;
                        s.file_total = snapshot.file_total;
                    }
                });
            }
            for handle in handles {
                if handle.join().is_err() && failure.is_none() {
                    failure = Some(anyhow!("Transfer worker thread panicked"));
                }
            }
        });
        metrics.peak_workers = metrics.peak_workers.max(peak.load(Ordering::SeqCst) as u32);
        self.progress.update(|s| {
            s.active_transfers = 0;
            s.completed_entries = done as u64;
        });
        if let Some(error) = failure {
            return Err(error);
        }
        crate::progress::check_cancel(Some(&cancel))?;
        Ok(())
    }
}
