//! Terminal road-stop construction and persisted joining.
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::{CommandMode, CommandRequest, runtime::SimulationRuntime};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn fixture() -> Result<SimulationRuntime> {
    let mut world = World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/replay/clear-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    world.edit_batch(vec![WorldEdit::Field {
        chunk: *b"DATE",
        record: 0,
        path: vec![PathElement::Field("pause_mode".into())],
        value: WireValue::Unsigned(0),
    }])?;
    Ok(SimulationRuntime::restore_vanilla(world)?)
}

fn request(tile: u32, stop_type: u8, station: u16) -> Result<CommandRequest> {
    Ok(serde_json::from_value(serde_json::json!({
        "company": 0, "mode": "post", "command": {
            "kind": "build_road_stop", "tile": tile, "stop": {
                "width": 1, "length": 1, "stop_type": stop_type,
                "drive_through": false, "direction": 0, "road_type": 0,
                "spec_class": 0, "spec_index": 0,
                "station_to_join": station, "adjacent": true
            }
        }
    }))?)
}

fn count(runtime: &SimulationRuntime, chunk: [u8; 4]) -> Result<usize> {
    Ok(runtime
        .world()
        .tables()
        .get(&chunk)
        .ok_or("table")?
        .records()
        .len())
}

#[test]
fn terminal_stop_allocates_station_and_join_survives_save() -> Result {
    // Given a clear native save with no stations.
    let mut runtime = fixture()?;
    assert_eq!(count(&runtime, *b"STNN")?, 0);
    // When constructing a new bus stop and restoring its saved world.
    let receipt = runtime.execute_command(&request(650, 0, 65533)?)?;
    assert!(receipt.posted);
    assert!(receipt.exec.ok_or("exec")?.success);
    assert_eq!(count(&runtime, *b"STNN")?, 1);
    assert_eq!(count(&runtime, *b"ROAD")?, 1);
    let tile = runtime.world().map().tiles().get(650).ok_or("tile")?;
    let station = tile.m2();
    assert_eq!(tile.tile_type() >> 4, 5);
    assert_eq!((tile.m6() >> 3) & 15, 3);
    let mut restored = SimulationRuntime::restore_vanilla(World::decode(&runtime.to_savegame()?)?)?;
    assert_eq!(
        restored.world().saved_json()?,
        runtime.world().saved_json()?
    );
    // Then joining a truck stop adds one ROAD row, preserving station identity.
    assert!(restored.execute_command(&request(651, 1, station)?)?.posted);
    assert_eq!(count(&restored, *b"STNN")?, 1);
    assert_eq!(count(&restored, *b"ROAD")?, 2);
    let joined = restored
        .world()
        .map()
        .tiles()
        .get(651)
        .ok_or("joined tile")?;
    assert_eq!(joined.m2(), station);
    assert_eq!((joined.m6() >> 3) & 15, 2);
    Ok(())
}

#[test]
fn terminal_stop_estimate_preserves_complete_world() -> Result {
    // Given a valid terminal stop request in estimate mode.
    let mut runtime = fixture()?;
    let mut estimate = request(650, 0, 65533)?;
    estimate.mode = CommandMode::Estimate;
    let saved = runtime.world().saved_json()?;
    let derived = runtime.world().derived().clone();
    // When estimating construction, no allocation, random draw or map edit escapes.
    let receipt = runtime.execute_command(&estimate)?;
    assert!(receipt.posted);
    assert!(receipt.exec.is_none());
    assert_eq!(runtime.world().saved_json()?, saved);
    assert_eq!(runtime.world().derived(), &derived);
    assert_eq!(count(&runtime, *b"STNN")?, 0);
    assert_eq!(count(&runtime, *b"ROAD")?, 0);
    Ok(())
}

#[test]
fn station_zero_x_native_rectangle_sentinel_is_explicitly_outside_domain() -> Result {
    let mut runtime = fixture()?;
    let before = runtime.world().saved_json()?;
    let derived = runtime.world().derived().clone();
    let pool = runtime.station_pool();
    assert!(matches!(
        runtime.execute_command(&request(0, 0, 65533)?),
        Err(ottd_sim::CommandError::Unsupported(
            "station rectangle sentinel edge"
        ))
    ));
    assert_eq!(runtime.world().saved_json()?, before);
    assert_eq!(runtime.world().derived(), &derived);
    assert_eq!(runtime.station_pool(), pool);
    Ok(())
}
