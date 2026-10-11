//! Original-valid industry boundaries must execute through the complete runtime.
use ottd_save::{Savegame, world::World};
use ottd_sim::runtime::SimulationRuntime;
use std::{fs::File, path::PathBuf};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "requires an original complete-loop industry capture"]
fn native_industry_complete_loop_matches_saved_world() -> Result {
    let directory = PathBuf::from(std::env::var("OTTD_INDUSTRY_NATIVE_DIR")?);
    let calls: u32 = std::env::var("OTTD_INDUSTRY_CALLS")?.parse()?;
    let world = World::decode(&Savegame::decode(
        &std::fs::read(directory.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let expected: serde_json::Value =
        serde_json::from_reader(File::open(directory.join("final.world.json"))?)?;
    assert_ne!(world.saved_json()?, expected);
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    runtime.advance_world(calls)?;
    assert_eq!(runtime.saved_json()?, expected);
    let derived: serde_json::Value =
        serde_json::from_reader(File::open(directory.join("final.derived.json"))?)?;
    assert_eq!(serde_json::to_value(runtime.world().derived())?, derived);
    Ok(())
}
