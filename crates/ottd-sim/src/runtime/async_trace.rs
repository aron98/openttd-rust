//! Optional main-thread diagnostics. No worker owners or scheduling controls are retained.
mod candidate;
mod model;
mod observe;
mod recorder;
#[cfg(test)]
mod tests;
pub(super) use candidate::Candidate;
pub(super) use observe::{job, sample, spawned};
use recorder::Recorder;
use std::{cell::RefCell, marker::PhantomData, path::Path, rc::Rc};

thread_local! {
    static RECORDER: RefCell<Option<Recorder>> = const { RefCell::new(None) };
}
/// Diagnostic failures, separate from gameplay errors.
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    /// A diagnostic session is already active on this thread.
    #[error("async trace session already active")]
    Active,
    /// The session was not available at finish.
    #[error("async trace session missing")]
    Missing,
    /// The retained trace contains invariant faults.
    #[error("async trace contains {0} diagnostic faults")]
    Faults(usize),
    /// Writing the diagnostic artifact failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Encoding the diagnostic artifact failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
/// A thread-bound recorder, outside all simulation graphs and ownership leases.
#[derive(Debug)]
pub struct Session {
    finished: bool,
    thread: PhantomData<Rc<()>>,
}
impl Session {
    /// Begin recording main-thread events. Call before the first observed advancement.
    /// # Errors
    /// Rejects a nested session without replacing its recorder.
    pub fn begin() -> Result<Self, TraceError> {
        RECORDER.with(|cell| {
            let mut slot = cell.borrow_mut();
            if slot.is_some() {
                return Err(TraceError::Active);
            }
            *slot = Some(Recorder::default());
            Ok(Self {
                finished: false,
                thread: PhantomData,
            })
        })
    }
    /// Mark the boundary immediately before an existing external snapshot.
    pub fn snapshot_before(&self) {
        with(|r| r.emit(Kind::SnapshotBefore, None));
    }
    /// Mark the boundary immediately after an existing external snapshot.
    pub fn snapshot_after(&self) {
        with(|r| r.emit(Kind::SnapshotAfter, None));
    }
    /// Write all events and diagnostic faults, then report any fault to the caller.
    /// # Errors
    /// Returns IO, encoding, missing-session or retained diagnostic errors.
    pub fn finish(mut self, path: &Path) -> Result<(), TraceError> {
        let recorder = RECORDER
            .with(|cell| cell.borrow_mut().take())
            .ok_or(TraceError::Missing)?;
        self.finished = true;
        let count = recorder.faults.len();
        let bytes = serde_json::to_vec_pretty(&serde_json::json!({
            "events": recorder.events, "faults": recorder.faults,
            "scope": "main-thread observed membership; no physical worker deallocation timestamps"
        }))?;
        std::fs::write(path, bytes)?;
        if count == 0 {
            Ok(())
        } else {
            Err(TraceError::Faults(count))
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if !self.finished {
            RECORDER.with(|cell| {
                cell.borrow_mut().take();
            });
            eprintln!("async trace session dropped without diagnostic finish");
        }
    }
}
fn with(action: impl FnOnce(&mut Recorder)) {
    RECORDER.with(|cell| {
        if let Some(recorder) = cell.borrow_mut().as_mut() {
            action(recorder);
        }
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    CandidateBegin,
    CandidatePrepared,
    CandidateCommitted,
    CandidateRefused,
    HostEnter,
    AfterPauseControl,
    InnerExit,
    InnerRefused,
    OwnerFirstSeen,
    WorkerSpawned,
    RunningAppended,
    WaitBegin,
    WaitReturned,
    PublicationBegin,
    PublicationEnd,
    StagedRemoval,
    CommittedRelease,
    CandidateDiscard,
    BranchSample,
    PauseApplied,
    SnapshotBefore,
    SnapshotAfter,
}

pub(crate) struct Host {
    finished: bool,
}
pub(crate) fn phase(kind: Kind, state: &crate::WorldTickState<'_>) {
    observe::phase(kind, state);
}
