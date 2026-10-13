use super::{after_load, before_tick};
use crate::runtime::{
    job_executor::{Worker, WorkerError},
    transport::TransportRuntime,
};
use ottd_save::{Savegame, world::World};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "requires genuine native fract19 prejoin save; held worker is Rust-only control"]
fn unfinished_due_worker_requests_pause_before_the_clock() -> Result {
    // Given an authentic ordinary prejoin world and a deliberately slow Rust worker.
    let bytes = std::fs::read(std::env::var("ASYNC_PREJOIN_INPUT")?)?;
    let mut world = World::decode(&Savegame::decode(&bytes, ottd_save::DEFAULT_MAX_BYTES)?)?;
    let mut transport =
        TransportRuntime::restore(&world, &crate::content::ContentCatalog::from_world(&world)?)?;
    let expected = transport
        .workers
        .get(&0)
        .ok_or("missing job0")?
        .wait()?
        .as_ref()
        .clone();
    let gate = Arc::new(AtomicBool::new(false));
    let input_gate = Arc::clone(&gate);
    let worker = Arc::new(Worker::spawn(move |cancel| {
        while !input_gate.load(Ordering::Acquire) {
            if cancel.load(Ordering::Acquire) {
                return Err(WorkerError::Cancelled);
            }
            std::thread::yield_now();
        }
        Ok(expected.clone())
    }));
    transport.workers.insert(0, Arc::clone(&worker));
    assert_eq!(
        crate::world_access::unsigned(&world, b"DATE", 0, "pause_mode")?,
        0
    );
    assert_eq!(
        crate::world_access::unsigned(&world, b"DATE", 0, "economy_date_fract")?,
        19
    );
    // When native pre-state-loop control runs before the clock.
    let industries = crate::runtime::IndustryCaches::restore(&world)?;
    let mut state = crate::WorldTickState::load(&mut world, None, industries);
    before_tick(&mut state, &transport)?;
    state.finish()?.0.commit();
    // Then pause is requested without advancing the saved game tick.
    assert_eq!(
        crate::world_access::unsigned(&world, b"DATE", 0, "pause_mode")?,
        64
    );
    assert_eq!(
        crate::world_access::unsigned(&world, b"DATE", 0, "tick_counter")?,
        3719
    );
    // Completion is deliberately released only after inspecting the pending branch.
    gate.store(true, Ordering::Release);
    worker.wait()?;
    let industries = crate::runtime::IndustryCaches::restore(&world)?;
    let mut state = crate::WorldTickState::load(&mut world, None, industries);
    before_tick(&mut state, &transport)?;
    state.finish()?.0.commit();
    assert_eq!(
        crate::world_access::unsigned(&world, b"DATE", 0, "pause_mode")?,
        0
    );
    // Afterload of already-completed work must not synthesize a pause.
    after_load(&mut world, &transport)?;
    assert_eq!(
        crate::world_access::unsigned(&world, b"DATE", 0, "pause_mode")?,
        0
    );
    let output = PathBuf::from(std::env::var("ASYNC_TEST_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    std::fs::write(
        output.join("controlled-prejoin.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "controlled_rust_worker_gate":true,"authentic_initial_tick":3719,"authentic_initial_fract":19,
            "pending_pre_tick_pause":64,"completed_pre_tick_pause":0,"completed_afterload_pause":0,
            "natural_native_prejoin_stall_claimed":false
        }))?,
    )?;
    Ok(())
}
