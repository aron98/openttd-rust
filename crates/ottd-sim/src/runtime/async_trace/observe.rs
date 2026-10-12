//! Borrow-only event capture from main-owned state; no worker polling.
use super::{
    Kind,
    model::{Clock, Fault},
    with,
};
use crate::WorldTickState;
use crate::runtime::{
    job_executor::Worker, transport::TransportRuntime, transport_jobs::JobResult,
};
use std::sync::Arc;

pub(super) fn token(worker: &Arc<Worker<JobResult>>) -> usize {
    Arc::as_ptr(worker).addr()
}
pub(super) fn phase(kind: Kind, state: &WorldTickState<'_>) {
    with(|recorder| {
        let clock = (|| {
            Ok::<_, crate::WorldTickError>(Clock {
                tick: state.number(b"DATE", 0, "tick_counter")?,
                economy_date: state.number(b"DATE", 0, "economy_date")?,
                economy_fract: state.number(b"DATE", 0, "economy_date_fract")?,
                pause: state.number(b"DATE", 0, "pause_mode")?,
            })
        })();
        if kind == Kind::HostEnter {
            recorder.host_enter();
        }
        if let Ok(clock) = clock {
            recorder.emit(kind, Some(clock));
        } else {
            recorder.faults.push(Fault::ClockRead);
            recorder.emit(kind, None);
        }
    });
}
pub(in crate::runtime) fn sample(
    owner: Option<(&Arc<Worker<JobResult>>, u32)>,
    due: Option<bool>,
    completed: Option<bool>,
) {
    with(|recorder| {
        recorder.sample(
            owner.map(|(worker, id)| (token(worker), id)),
            due,
            completed,
        );
    });
}
pub(in crate::runtime) fn job(transport: &TransportRuntime, id: u32, kind: Kind) {
    with(|recorder| {
        if let Some(worker) = transport.workers.get(&id) {
            recorder.job(token(worker), id, kind);
        } else {
            recorder.faults.push(Fault::UnknownOwner);
        }
    });
}
pub(in crate::runtime) fn spawned(transport: &TransportRuntime, id: u32) {
    with(|recorder| {
        if let Some(worker) = transport.workers.get(&id) {
            recorder.attach(token(worker), id, Kind::WorkerSpawned);
        } else {
            recorder.faults.push(Fault::UnknownOwner);
        }
    });
}
