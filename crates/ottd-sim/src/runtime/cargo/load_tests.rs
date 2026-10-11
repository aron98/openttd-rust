//! Native packet/list effects isolated from clock, payment and vehicle-trigger effects.
use super::{CargoRuntime, load::Load};
use crate::{WorldTickState, runtime::VehicleId};
use ottd_save::{Savegame, world::World};
use std::path::PathBuf;
type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
#[ignore = "requires authentic original adjacent loading steps and output path"]
fn original_packet_load_and_split_effects() -> Result {
    let input = std::env::var("CARGO_INPUT")?;
    let native = PathBuf::from(std::env::var("CARGO_NATIVE")?);
    let output = PathBuf::from(std::env::var("CARGO_OUTPUT")?);
    let station = std::env::var("CARGO_STATION")?.parse::<u32>()?;
    let tile = std::env::var("CARGO_TILE")?.parse::<u32>()?;
    let mut world = World::decode(&Savegame::decode(
        &std::fs::read(input)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let mut cargo = CargoRuntime::restore(&world)?;
    let mut state = WorldTickState::load(&mut world, None);
    let loaded = cargo.load(
        &mut state,
        Load {
            station,
            cargo: 0,
            vehicle: VehicleId::new(0),
            amount: 5,
            tile,
            next: &[u16::try_from(1 - station)?],
        },
    )?;
    assert_eq!(loaded, 5);
    cargo.validate(state.view())?;
    let (candidate, _) = state.finish()?;
    candidate.commit();
    let expected: serde_json::Value =
        serde_json::from_slice(&std::fs::read(native.join("final.cargo.json"))?)?;
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(native.join("final.world.json"))?)?;
    let actual = world.saved_json()?;
    std::fs::create_dir_all(&output)?;
    std::fs::write(
        output.join("actual-cargo.json"),
        serde_json::to_vec_pretty(&cargo.snapshot())?,
    )?;
    std::fs::write(
        output.join("actual-world.json"),
        serde_json::to_vec(&actual)?,
    )?;
    assert_eq!(
        cargo.snapshot(),
        expected,
        "full cargo runtime after isolated loading effects"
    );
    assert_eq!(
        actual.pointer("/chunks/CAPA"),
        saved.pointer("/chunks/CAPA"),
        "complete native packet pool saved data"
    );
    std::fs::write(output.join("result.json"), b"{\"loaded\":5,\"full_cargo_cache_equal\":true,\"saved_packet_pool_equal\":true,\"full_tick_parity\":false}")?;
    Ok(())
}
