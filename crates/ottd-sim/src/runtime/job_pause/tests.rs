use super::after_load;
use crate::runtime::{
    SimulationRuntime,
    job_executor::{Worker, WorkerError},
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
fn load(variable: &str) -> Result<SimulationRuntime> {
    let bytes = std::fs::read(std::env::var(variable)?)?;
    Ok(SimulationRuntime::restore_vanilla(World::decode(
        &Savegame::decode(&bytes, ottd_save::DEFAULT_MAX_BYTES)?,
    )?)?)
}

#[test]
#[ignore = "requires genuine native due-date save; held worker is labelled Rust-only scheduling control"]
fn due_restore_pauses_zero_calls_preserve_it_and_completed_work_releases_it() -> Result {
    // Given a genuine due-date world and an explicitly held Rust worker.
    let mut runtime = load("ASYNC_DUE_INPUT")?;
    let result = runtime
        .transport
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
        Ok(result.clone())
    }));
    runtime.transport.workers.insert(0, Arc::clone(&worker));
    after_load(&mut runtime.world, &runtime.transport)?;
    let paused_world = runtime.saved_json()?;
    let before = format!("{runtime:?}");
    assert_eq!(
        crate::world_access::unsigned(&runtime.world, b"DATE", 0, "pause_mode")?,
        64
    );
    // When zero calls, paused calls, and then a completed-worker call execute.
    let zero = runtime.advance_world(0)?;
    assert!(zero.ticks.is_empty());
    assert_eq!(before, format!("{runtime:?}"));
    assert_eq!(paused_world, runtime.saved_json()?);
    runtime.advance_world(2)?;
    assert_eq!(paused_world, runtime.saved_json()?);
    gate.store(true, Ordering::Release);
    worker.wait()?;
    runtime.advance_world(1)?;
    // Then the actual source pause path releases and advances exactly one game tick.
    assert_eq!(
        crate::world_access::unsigned(&runtime.world, b"DATE", 0, "pause_mode")?,
        0
    );
    assert_eq!(
        crate::world_access::unsigned(&runtime.world, b"DATE", 0, "tick_counter")?,
        3405
    );
    let native: serde_json::Value =
        serde_json::from_slice(&std::fs::read(std::env::var("ASYNC_NATIVE_AFTER")?)?)?;
    assert_eq!(native, runtime.saved_json()?);
    let output = PathBuf::from(std::env::var("ASYNC_TEST_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    std::fs::write(
        output.join("controlled-pause.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "controlled_rust_worker_gate":true,"zero_call_unchanged":true,"two_paused_calls_world_unchanged":true,
            "released_call_complete_native_world_equal":true,"native_initial_pause_witness":"due-restart-native/native/initial.world.json"
        }))?,
    )?;
    Ok(())
}

#[test]
#[ignore = "requires genuine command-created original-settings road route"]
fn late_native_breakdown_refusal_rolls_back_provisional_async_jobs() -> Result {
    // Given the authentic original-settings route with no preexisting workers.
    let mut runtime = load("ASYNC_ROUTE_INPUT")?;
    assert!(runtime.transport.workers.is_empty());
    let world = runtime.saved_json()?;
    let before = format!("{runtime:?}");
    // When a long transaction creates/joins jobs before reaching unsupported breakdown work.
    let error = runtime
        .advance_world(6000)
        .expect_err("breakdown family is still required");
    // Then every committed field/cache/worker ownership remains unchanged.
    assert!(error.to_string().contains("road_day"), "{error}");
    assert_eq!(world, runtime.saved_json()?);
    assert_eq!(before, format!("{runtime:?}"));
    assert!(runtime.transport.workers.is_empty());
    let output = PathBuf::from(std::env::var("ASYNC_TEST_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    std::fs::write(
        output.join("late-breakdown-rollback.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "error":error.to_string(),"complete_world_unchanged":true,"committed_runtime_unchanged":true,
            "committed_worker_ownership_unchanged":true,"provisional_cleanup":"sole owner Drop joins before return"
        }))?,
    )?;
    Ok(())
}

#[test]
#[ignore = "requires genuine due-date job and original-settings route"]
fn late_refusal_after_join_keeps_preexisting_worker_handle_owned() -> Result {
    // Given a real restored worker retained by committed runtime ownership.
    let mut runtime = load("ASYNC_DUE_INPUT")?;
    let owner = Arc::clone(runtime.transport.workers.get(&0).ok_or("missing worker0")?);
    owner.wait()?;
    assert_eq!(
        owner.backend(),
        crate::runtime::job_executor::Backend::Threaded
    );
    assert_ne!(owner.executing_thread(), Some(std::thread::current().id()));
    assert!(owner.executing_thread().is_some());
    let before = format!("{runtime:?}");
    let world = runtime.saved_json()?;
    // When the candidate waits/joins its job but later reaches breakdown work.
    let error = runtime
        .advance_world(3000)
        .expect_err("breakdown remains unsupported");
    // Then candidate cleanup has neither consumed nor replaced the committed handle owner.
    assert!(error.to_string().contains("road_day"), "{error}");
    assert!(Arc::ptr_eq(
        &owner,
        runtime.transport.workers.get(&0).ok_or("lost worker")?
    ));
    assert_eq!(Arc::strong_count(&owner), 2);
    assert_eq!(before, format!("{runtime:?}"));
    assert_eq!(world, runtime.saved_json()?);
    assert!(owner.poll()?.is_some());
    let output = PathBuf::from(std::env::var("ASYNC_TEST_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    std::fs::write(
        output.join("preexisting-worker-rollback.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "error":error.to_string(),"complete_world_unchanged":true,"committed_runtime_unchanged":true,
            "actual_worker_backend":"Threaded", "worker_thread":format!("{:?}",owner.executing_thread()),
        "caller_thread":format!("{:?}",std::thread::current().id()),
        "preexisting_worker_arc_identity_unchanged":true,"owners_after_failure":2,"result_still_readable":true
        }))?,
    )?;
    Ok(())
}

#[test]
fn opaque_job_command_context_is_explicit_and_cannot_execute_pause_control() -> Result {
    // Given the established populated command fixture, not a supported solver fixture.
    let (mut runtime, _) = crate::runtime::purchase_tests::fixture::fixture()?;
    let snapshot = runtime.transport_state_json();
    let opaque = snapshot
        .get("unsupported_jobs")
        .ok_or("opaque job field")?
        .as_array()
        .ok_or("missing opaque job diagnostic")?;
    assert!(!opaque.is_empty());
    for id in opaque {
        let key = id.as_u64().ok_or("opaque identifier")?.to_string();
        assert!(
            snapshot
                .get("job_results")
                .ok_or("job results field")?
                .get(&key)
                .is_none()
        );
    }
    let world = runtime.saved_json()?;
    let before = format!("{runtime:?}");
    // When the simulation-specific pre-tick consumer is requested.
    let mut state =
        crate::WorldTickState::load(&mut runtime.world, None, runtime.industries.clone());
    let error =
        super::before_tick(&mut state, &runtime.transport).expect_err("opaque jobs must not run");
    drop(state);
    // Then command restoration remains available but execution refuses transactionally.
    assert!(
        error.to_string().contains("populated link job execution"),
        "{error}"
    );
    assert_eq!(world, runtime.saved_json()?);
    assert_eq!(before, format!("{runtime:?}"));
    Ok(())
}
