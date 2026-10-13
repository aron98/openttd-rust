//! Genuine original-command houses exercised through the complete saved-world runtime.
use ottd_save::{Savegame, world::World};
use ottd_sim::runtime::SimulationRuntime;
use std::{fs::File, path::PathBuf};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "requires original-command populated house capture"]
fn native_houses_match_direct_complete_world_ticks() -> Result {
    let directory = PathBuf::from(std::env::var("OTTD_HOUSE_NATIVE_DIR")?);
    let mut world = World::decode(&Savegame::decode(
        &std::fs::read(directory.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    ottd_sim::advance_world(&mut world, 256)?;
    let expected: serde_json::Value =
        serde_json::from_reader(File::open(directory.join("final.world.json"))?)?;
    assert_eq!(world.saved_json()?, expected);
    Ok(())
}

#[test]
#[ignore = "requires original-command populated house capture"]
fn native_houses_match_complete_world_ticks() -> Result {
    let directory = PathBuf::from(std::env::var("OTTD_HOUSE_NATIVE_DIR")?);
    let world = World::decode(&Savegame::decode(
        &std::fs::read(directory.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    runtime.advance_world(256)?;
    let expected: serde_json::Value =
        serde_json::from_reader(File::open(directory.join("final.world.json"))?)?;
    assert_eq!(runtime.saved_json()?, expected);
    let derived: serde_json::Value =
        serde_json::from_reader(File::open(directory.join("final.derived.json"))?)?;
    assert_eq!(serde_json::to_value(runtime.world().derived())?, derived);
    Ok(())
}
