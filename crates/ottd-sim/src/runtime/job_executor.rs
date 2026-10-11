//! Owned host workers for immutable link-graph computations.
use std::{
    fmt,
    panic::{RefUnwindSafe, catch_unwind},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Backend {
    Threaded,
    SynchronousFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(super) enum WorkerError {
    #[error("link graph worker panicked")]
    Panicked,
    #[error("link graph worker result synchronization failed")]
    Synchronization,
    #[error("link graph worker cancelled")]
    Cancelled,
}

type Outcome<T> = Result<Arc<T>, WorkerError>;
struct Completion<T> {
    result: Mutex<Option<Outcome<T>>>,
    ready: Condvar,
    cancelled: AtomicBool,
    #[cfg(test)]
    started: std::sync::OnceLock<thread::ThreadId>,
}

/// A handle has one owner. Candidate leases share that owner, never the handle itself.
pub(super) struct Worker<T> {
    completion: Arc<Completion<T>>,
    handle: Option<JoinHandle<()>>,
    backend: Backend,
}
impl<T> fmt::Debug for Worker<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Live completion is a separate observation; it is not committed simulation state.
        f.debug_struct("Worker")
            .field("backend", &self.backend)
            .finish_non_exhaustive()
    }
}
impl<T: Send + Sync + 'static> Worker<T> {
    pub(super) fn spawn<F>(task: F) -> Self
    where
        F: Fn(&AtomicBool) -> Result<T, WorkerError> + Send + Sync + RefUnwindSafe + 'static,
    {
        let completion = Arc::new(Completion {
            result: Mutex::new(None),
            ready: Condvar::new(),
            cancelled: AtomicBool::new(false),
            #[cfg(test)]
            started: std::sync::OnceLock::new(),
        });
        let task = Arc::new(task);
        let run_task = Arc::clone(&task);
        let output = Arc::clone(&completion);
        if let Ok(handle) = thread::Builder::new()
            .name("ottd:linkgraph".into())
            .spawn(move || execute(&output, run_task.as_ref()))
        {
            Self {
                completion,
                handle: Some(handle),
                backend: Backend::Threaded,
            }
        } else {
            // Native SpawnThread runs synchronously only when host thread creation fails.
            execute(&completion, task.as_ref());
            Self {
                completion,
                handle: None,
                backend: Backend::SynchronousFallback,
            }
        }
    }
}
impl<T> Worker<T> {
    #[cfg(test)]
    pub(super) const fn backend(&self) -> Backend {
        self.backend
    }
    #[cfg(test)]
    pub(super) fn executing_thread(&self) -> Option<thread::ThreadId> {
        self.completion.started.get().copied()
    }
    pub(super) fn poll(&self) -> Result<Option<Arc<T>>, WorkerError> {
        self.completion
            .result
            .lock()
            .map_err(|_| WorkerError::Synchronization)?
            .as_ref()
            .cloned()
            .transpose()
    }
    /// Wait for immutable publication without consuming a committed owner's thread handle.
    pub(super) fn wait(&self) -> Outcome<T> {
        let mut result = self
            .completion
            .result
            .lock()
            .map_err(|_| WorkerError::Synchronization)?;
        loop {
            if let Some(outcome) = result.as_ref() {
                let outcome = outcome.clone();
                drop(result);
                // Native destruction joins before flow publication. A candidate cannot consume
                // an existing owner's handle, but can await termination without taking it.
                if let Some(handle) = &self.handle {
                    while !handle.is_finished() {
                        thread::yield_now();
                    }
                }
                return outcome;
            }
            result = self
                .completion
                .ready
                .wait(result)
                .map_err(|_| WorkerError::Synchronization)?;
        }
    }
}
impl<T> Drop for Worker<T> {
    fn drop(&mut self) {
        // Completed scheduled work is retired without aborting its published result.
        // An unfinished owner is being discarded (rollback/runtime cleanup).
        let pending = self
            .completion
            .result
            .lock()
            .map_or(true, |result| result.is_none());
        if pending {
            self.completion.cancelled.store(true, Ordering::Release);
        }
        if let Some(handle) = self.handle.take() {
            if handle.join().is_err() {
                eprintln!("link graph worker failed outside its computation boundary");
            }
        }
    }
}
fn execute<T, F>(completion: &Completion<T>, task: &F)
where
    F: Fn(&AtomicBool) -> Result<T, WorkerError> + RefUnwindSafe,
{
    #[cfg(test)]
    completion.started.get_or_init(|| thread::current().id());
    let result = catch_unwind(|| task(&completion.cancelled))
        .map_err(|_| WorkerError::Panicked)
        .and_then(|result| result)
        .map(Arc::new);
    match completion.result.lock() {
        Ok(mut slot) => {
            *slot = Some(result);
        }
        Err(poisoned) => {
            *poisoned.into_inner() = Some(Err(WorkerError::Synchronization));
        }
    }
    completion.ready.notify_all();
}
#[cfg(test)]
mod tests;
