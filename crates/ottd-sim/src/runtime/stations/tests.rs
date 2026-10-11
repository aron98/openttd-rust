use super::*;
use crate::{CommandRequest, runtime::SimulationRuntime};
use ottd_save::Savegame;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn fixture() -> Result<SimulationRuntime> {
    Ok(SimulationRuntime::restore_vanilla(World::decode(
        &Savegame::decode(
            include_bytes!("../../../../../fixtures/road-orders/stations-v362.sav"),
            ottd_save::DEFAULT_MAX_BYTES,
        )?,
    )?)?)
}
fn join() -> Result<CommandRequest> {
    Ok(serde_json::from_value(
        serde_json::json!({"company":0,"mode":"post","command":{
        "kind":"build_road_stop","tile":651,"stop":{"width":1,"length":1,"stop_type":0,
        "drive_through":false,"direction":0,"road_type":0,"spec_class":0,"spec_index":0,
        "station_to_join":0,"adjacent":true}}}),
    )?)
}
#[test]
fn late_station_pool_failure_preserves_world_rng_and_all_runtime_caches() -> Result {
    let mut runtime = fixture()?;
    runtime.stations.stops.insert(63000)?;
    let saved = runtime.world().saved_json()?;
    let derived = runtime.world().derived().clone();
    let stations = runtime.stations.clone();
    let depot = runtime.depot.clone();
    let orders = runtime.orders.clone();
    let road = runtime.road.clone();
    let allocation = runtime.allocation.clone();
    assert!(matches!(
        runtime.execute_command(&join()?),
        Err(crate::CommandError::Runtime(RuntimeError::Invalid(
            "station allocation membership"
        )))
    ));
    assert_eq!(runtime.world().saved_json()?, saved);
    assert_eq!(runtime.world().derived(), &derived);
    assert_eq!(runtime.stations, stations);
    assert_eq!(runtime.depot, depot);
    assert_eq!(runtime.orders, orders);
    assert_eq!(runtime.road, road);
    assert_eq!(runtime.allocation, allocation);
    Ok(())
}
#[test]
fn late_station_cache_failure_does_not_publish_joined_chain_or_map() -> Result {
    let mut runtime = fixture()?;
    runtime
        .stations
        .geometry
        .get_mut(&0)
        .ok_or("station")?
        .catchment
        .insert(4095);
    let saved = runtime.world().saved_json()?;
    let derived = runtime.world().derived().clone();
    let stations = runtime.stations.clone();
    let depot = runtime.depot.clone();
    assert!(matches!(
        runtime.execute_command(&join()?),
        Err(crate::CommandError::Runtime(RuntimeError::Invalid(
            "station candidate geometry"
        )))
    ));
    assert_eq!(runtime.world().saved_json()?, saved);
    assert_eq!(runtime.world().derived(), &derived);
    assert_eq!(runtime.stations, stations);
    assert_eq!(runtime.depot, depot);
    Ok(())
}
#[test]
fn loaded_road_stop_status_and_geometry_survive_resaving() -> Result {
    let runtime = fixture()?;
    let restored = SimulationRuntime::restore_vanilla(World::decode(&runtime.to_savegame()?)?)?;
    assert_eq!(runtime.stations, restored.stations);
    assert_eq!(runtime.depot, restored.depot);
    assert_eq!(
        runtime.world().saved_json()?,
        restored.world().saved_json()?
    );
    Ok(())
}
