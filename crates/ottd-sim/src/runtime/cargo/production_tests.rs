//! Source-identified native town production, isolated from the enclosing clock and RNG.
use crate::{
    WorldTickState,
    cargo::{CargoSource, Production, TileArea},
    runtime::{stations::StationRuntime, transport::TransportRuntime},
};
use ottd_save::{Savegame, world::World};
use std::path::PathBuf;
type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
#[ignore = "requires actual original first-production boundary and output directory"]
fn original_town_packet_distribution_effects() -> Result {
    let native = PathBuf::from(std::env::var("CARGO_NATIVE")?);
    let output = PathBuf::from(std::env::var("CARGO_OUTPUT")?);
    let mut world = World::decode(&Savegame::decode(
        &std::fs::read(native.join("before.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let stations = StationRuntime::restore(&world)?;
    let mut transport = TransportRuntime::restore(&world)?;
    let mut state = WorldTickState::load(&mut world, None);
    let moved = transport.town_production(
        &mut state,
        Production {
            source: CargoSource::Town(0),
            area: TileArea {
                tile: 1958,
                width: 1,
                height: 1,
            },
            cargo: 0,
            amount: 2,
        },
        &stations,
    )?;
    assert_eq!(moved, 1);
    transport.cargo.validate(state.view())?;
    let (candidate, _) = state.finish()?;
    candidate.commit();
    let expected: serde_json::Value =
        serde_json::from_slice(&std::fs::read(native.join("after.cargo.json"))?)?;
    let expected_world: serde_json::Value =
        serde_json::from_slice(&std::fs::read(native.join("after.world.json"))?)?;
    let actual = world.saved_json()?;
    std::fs::create_dir_all(&output)?;
    std::fs::write(
        output.join("actual-cargo.json"),
        serde_json::to_vec_pretty(&transport.cargo.snapshot())?,
    )?;
    std::fs::write(
        output.join("actual-world.json"),
        serde_json::to_vec(&actual)?,
    )?;
    assert_eq!(
        transport.cargo.snapshot(),
        expected,
        "complete cargo runtime after source distribution"
    );
    for path in [
        "/chunks/CAPA",
        "/chunks/LGRP",
        "/chunks/STNN/records/0/normal/0/goods",
        "/chunks/STNN/records/1/normal/0/goods",
    ] {
        assert_eq!(
            actual.pointer(path),
            expected_world.pointer(path),
            "native production effects {path}"
        );
    }
    std::fs::write(output.join("result.json"),b"{\"produced_input\":2,\"actual_moved\":1,\"cargo_cache_packets_graphs_goods_equal\":true,\"full_tick_parity\":false}")?;
    Ok(())
}
