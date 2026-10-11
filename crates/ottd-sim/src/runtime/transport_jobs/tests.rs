use super::*;
use std::time::{Duration, Instant};

#[test]
fn cleanup_cancels_all_solely_owned_workers_before_joining_any()
-> Result<(), Box<dyn std::error::Error>> {
    // Given two provisional workers which acknowledge cancellation together.
    let (runtime, _) = crate::runtime::purchase_tests::fixture::fixture()?;
    let mut transport = runtime.transport;
    transport.workers.clear();
    let flags = Arc::new([AtomicBool::new(false), AtomicBool::new(false)]);
    let timed_out = Arc::new(AtomicBool::new(false));
    for id in 0_u32..2 {
        let flags = Arc::clone(&flags);
        let timeout = Arc::clone(&timed_out);
        let index = usize::try_from(id)?;
        let worker = Arc::new(Worker::spawn(move |cancel| {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !flags.iter().all(|flag| flag.load(Ordering::Acquire)) {
                if cancel.load(Ordering::Acquire) {
                    flags.get(index).unwrap().store(true, Ordering::Release);
                }
                if Instant::now() >= deadline {
                    timeout.store(true, Ordering::Release);
                    break;
                }
                std::thread::yield_now();
            }
            Ok(EmptyJob { nodes: 0, edges: 0 })
        }));
        transport.workers.insert(id, worker);
    }
    // When cleanup discards their sole ownership without publishing flows.
    drop(transport);
    // Then both jobs were aborted before any blocking join, as native Clear requires.
    assert!(!timed_out.load(Ordering::Acquire));
    assert!(flags.iter().all(|flag| flag.load(Ordering::Acquire)));
    Ok(())
}
