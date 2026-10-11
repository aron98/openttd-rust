//! Labelled cache fault after a genuine successful station-publication prefix.
use crate::{CommandRequest, runtime::SimulationRuntime};
use ottd_save::{Savegame, world::World};
use std::path::PathBuf;
type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn fixture() -> std::result::Result<SimulationRuntime, Box<dyn std::error::Error>> {
    Ok(SimulationRuntime::restore_vanilla(World::decode(
        &Savegame::decode(
            include_bytes!("../../../../../fixtures/road-orders/stations-v362.sav"),
            ottd_save::DEFAULT_MAX_BYTES,
        )?,
    )?)?)
}

#[test]
#[ignore = "requires explicit output for the labelled cargo cache fault receipt"]
fn cargo_command_late_pool_failure_is_atomic() -> Result {
    let request: CommandRequest = serde_json::from_value(serde_json::json!({
        "company":0,"mode":"post","command":{"kind":"build_road_stop","tile":651,
        "stop":{"width":1,"length":1,"stop_type":0,"drive_through":false,"direction":0,
        "road_type":0,"spec_class":0,"spec_index":0,"station_to_join":0,"adjacent":true}}
    }))?;
    let output = PathBuf::from(std::env::var("CARGO_FAULT_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    let mut positive = fixture()?;
    let original = positive.saved_json()?;
    assert!(positive.execute_command(&request)?.posted);
    assert_ne!(
        positive.saved_json()?,
        original,
        "prefix must mutate real station state"
    );
    let mut fault = fixture()?;
    fault.transport.cargo.pool.insert(10_000)?;
    let before = format!("{fault:?}");
    let saved = fault.saved_json()?;
    let derived = serde_json::to_value(fault.world().derived())?;
    let result = fault.execute_command(&request);
    let message = format!("{result:?}");
    assert!(
        message.contains("candidate cargo pool membership"),
        "{message}"
    );
    assert_eq!(format!("{fault:?}"), before);
    assert_eq!(fault.saved_json()?, saved);
    assert_eq!(serde_json::to_value(fault.world().derived())?, derived);
    std::fs::write(
        output.join("receipt.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "label":"synthetic extra cargo-pool identity after genuine positive station prefix",
            "positive_command_posted":true,"positive_world_mutated":true,
            "late_error":message,"complete_saved_world_unchanged":true,
            "complete_derived_world_unchanged":true,"complete_runtime_unchanged":true
        }))?,
    )?;
    Ok(())
}
