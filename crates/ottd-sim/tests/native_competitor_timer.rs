//! Original zero-AI timer controls through the complete saved-world loop.
use ottd_save::{Savegame, world::World};
use serde::Deserialize;
use std::{fs::File, path::PathBuf};

#[path = "industry_recipient_support/native_views.rs"]
mod native_views;
include!("disaster_support/mod.rs");
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Protocol {
    schema_version: u8,
    actions: Vec<Tick>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Tick {
    ordinal: u32,
    op: String,
    count: u32,
}

#[test]
#[ignore = "requires original zero-AI timer control with one whole-loop tick action"]
fn native_zero_ai_timer_matches_complete_world_loop() -> Result {
    let directory = PathBuf::from(std::env::var("OTTD_COMPETITOR_NATIVE_DIR")?);
    let protocol: Protocol = serde_json::from_reader(File::open(directory.join("protocol.json"))?)?;
    assert_eq!(protocol.schema_version, 1);
    assert_eq!(protocol.actions.len(), 1);
    let tick = protocol.actions.first().ok_or("tick action")?;
    assert_eq!(tick.ordinal, 0);
    assert_eq!(tick.op, "tick");
    let mut world = World::decode(&Savegame::decode(
        &std::fs::read(directory.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    ottd_sim::advance_world(&mut world, tick.count)?;
    let expected: serde_json::Value =
        serde_json::from_reader(File::open(directory.join("final.world.json"))?)?;
    assert_eq!(world.saved_json()?, expected);
    let derived: serde_json::Value =
        serde_json::from_reader(File::open(directory.join("final.derived.json"))?)?;
    assert_eq!(serde_json::to_value(world.derived())?, derived);
    Ok(())
}

#[test]
#[ignore = "requires actual 60680-call original capture; historical refusal selector retained without rollback credit"]
fn late_disaster_boundary_rolls_back_prior_script_random_reset() -> Result {
    let directory = PathBuf::from(std::env::var("OTTD_COMPETITOR_NATIVE_DIR")?);
    let save = Savegame::decode(
        &std::fs::read(directory.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?;
    let input = World::decode(&save)?;
    let capture = DisasterCapture::required(&input)?;
    let mut runtime = ottd_sim::runtime::SimulationRuntime::restore_vanilla(input.clone())?;
    capture.compare(&runtime, "initial")?;
    let mut prefix = World::decode(&save)?;
    let before = prefix.saved_json()?;
    ottd_sim::advance_world(&mut prefix, 1)?;
    assert_ne!(
        before.pointer("/chunks/SRND/records/16"),
        prefix.saved_json()?.pointer("/chunks/SRND/records/16")
    );
    runtime.advance_world(1)?;
    assert_eq!(runtime.saved_json()?, prefix.saved_json()?);
    capture.compare(&runtime, "prefix")?;
    let mut runtime = ottd_sim::runtime::SimulationRuntime::restore_vanilla(input)?;
    runtime.advance_world(60_680)?;
    capture.finish(&runtime, 1)?;
    let mut world = World::decode(&save)?;
    ottd_sim::advance_world(&mut world, 60_680)?;
    assert_eq!(world.saved_json()?, runtime.saved_json()?);
    assert_eq!(world.derived(), runtime.world().derived());
    Ok(())
}
