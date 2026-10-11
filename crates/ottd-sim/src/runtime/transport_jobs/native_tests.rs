//! Genuine original saved jobs: compare the complete completed annotation state.
use crate::runtime::{SimulationRuntime, job_executor::Backend};
use ottd_save::{Savegame, world::World};
use std::path::PathBuf;

#[test]
#[ignore = "requires genuine original job save and completed annotation observation"]
fn original_manual_completed_annotations() -> Result<(), Box<dyn std::error::Error>> {
    let input = PathBuf::from(std::env::var("JOB_INPUT")?);
    let output = PathBuf::from(std::env::var("JOB_OUTPUT")?);
    let expected: serde_json::Value =
        serde_json::from_slice(&std::fs::read(std::env::var("JOB_EXPECTED")?)?)?;
    let runtime = SimulationRuntime::restore_vanilla(World::decode(&Savegame::decode(
        &std::fs::read(input)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)?;
    std::fs::create_dir_all(&output)?;
    std::fs::write(
        output.join("restored-transport.json"),
        serde_json::to_vec_pretty(&runtime.transport_state_json())?,
    )?;
    let worker = runtime
        .transport
        .workers
        .get(&0)
        .ok_or("manual worker missing")?;
    let actual = worker.wait()?;
    assert_eq!(worker.backend(), Backend::Threaded);
    assert_ne!(worker.executing_thread(), Some(std::thread::current().id()));
    let actual = serde_json::to_value(actual.as_ref())?;
    std::fs::write(
        output.join("completed.json"),
        serde_json::to_vec_pretty(&actual)?,
    )?;
    assert_eq!(
        actual, expected,
        "complete original completed annotation view"
    );
    std::fs::write(output.join("PASS.json"), b"{\"complete_annotations\":true,\"actual_threaded_worker\":true,\"full_tick_parity\":false}")?;
    Ok(())
}
