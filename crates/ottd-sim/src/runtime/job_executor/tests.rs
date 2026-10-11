use super::{Backend, Worker, WorkerError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

#[test]
fn worker_runs_on_another_thread_and_publishes_only_finished_result() {
    // Given an actual worker whose input gate is controlled only by this unit test.
    let main = std::thread::current().id();
    let gate = Arc::new(AtomicBool::new(false));
    let input_gate = Arc::clone(&gate);
    let (started, witness) = mpsc::sync_channel(1);
    let worker = Worker::spawn(move |cancel| {
        started.send(std::thread::current().id()).unwrap();
        while !input_gate.load(Ordering::Acquire) && !cancel.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        Ok(42_u32)
    });
    // When the real thread starts before its computation is allowed to finish.
    let thread = witness.recv_timeout(Duration::from_secs(5)).unwrap();
    // Then observation is nonblocking and exposes no partial result.
    assert_eq!(worker.backend(), Backend::Threaded);
    assert_ne!(main, thread);
    assert_eq!(worker.executing_thread(), Some(thread));
    assert_eq!(worker.poll().unwrap(), None);
    gate.store(true, Ordering::Release);
    assert_eq!(*worker.wait().unwrap(), 42);
    assert_eq!(*worker.poll().unwrap().unwrap(), 42);
}

#[test]
fn failed_candidate_lease_does_not_cancel_or_consume_existing_worker() {
    // Given one committed owner and a candidate lease.
    let gate = Arc::new(AtomicBool::new(false));
    let input_gate = Arc::clone(&gate);
    let (started, witness) = mpsc::sync_channel(1);
    let owner = Arc::new(Worker::spawn(move |cancel| {
        started.send(()).unwrap();
        while !input_gate.load(Ordering::Acquire) {
            assert!(!cancel.load(Ordering::Acquire));
            std::thread::yield_now();
        }
        Ok(7_u32)
    }));
    witness.recv_timeout(Duration::from_secs(5)).unwrap();
    let candidate = Arc::clone(&owner);
    // When the candidate is discarded without publishing.
    drop(candidate);
    // Then committed ownership is intact and a later lease can receive its result.
    assert_eq!(Arc::strong_count(&owner), 1);
    assert_eq!(owner.poll().unwrap(), None);
    gate.store(true, Ordering::Release);
    assert_eq!(*owner.wait().unwrap(), 7);
}

#[test]
fn provisional_owner_drop_cancels_and_joins_before_returning() {
    // Given a provisional worker doing cancellable work.
    let (started, witness) = mpsc::sync_channel(1);
    let finished = Arc::new(AtomicBool::new(false));
    let exited = Arc::clone(&finished);
    let worker = Worker::spawn(move |cancel| {
        started.send(()).unwrap();
        while !cancel.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        exited.store(true, Ordering::Release);
        Err::<(), _>(WorkerError::Cancelled)
    });
    witness.recv_timeout(Duration::from_secs(5)).unwrap();
    // When rollback drops its sole owner.
    drop(worker);
    // Then there is no orphan worker left after cleanup returns.
    assert!(finished.load(Ordering::Acquire));
}

#[test]
fn waiting_candidate_does_not_consume_result_needed_after_rollback() {
    // Given shared committed ownership of a real worker.
    let owner = Arc::new(Worker::spawn(|_| Ok(19_u32)));
    let candidate = Arc::clone(&owner);
    // When a candidate waits at a join barrier and is then discarded.
    assert_eq!(*candidate.wait().unwrap(), 19);
    drop(candidate);
    // Then another candidate can still observe the same immutable result.
    assert_eq!(*owner.wait().unwrap(), 19);
    assert_eq!(Arc::strong_count(&owner), 1);
}

#[test]
fn panicking_worker_publishes_typed_failure_instead_of_hanging_waiter() {
    // Given a computation which panics on its own host thread.
    let worker = Worker::<()>::spawn(|_| panic!("deliberate worker failure"));
    // When a scheduled consumer waits for it.
    let result = worker.wait();
    // Then failure is explicit and the owned handle remains safe to join on drop.
    assert_eq!(result, Err(WorkerError::Panicked));
}
