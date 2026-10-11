//! Native command-created route must complete its whole requested round trip.
use ottd_save::{Compression, Savegame, world::World};
use ottd_sim::runtime::SimulationRuntime;
use std::path::PathBuf;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "requires command-only original route discovery and observed horizon"]
fn native_terminal_round_trip() -> Result {
    run_horizon()
}

#[test]
#[ignore = "requires genuine stopped route and original station phase proof"]
fn native_idle_station_phase() -> Result {
    run_horizon()
}

fn run_horizon() -> Result {
    // Given: a genuine original prepared route, with orders and a started vehicle.
    let input = PathBuf::from(std::env::var("ROAD_ROUTE_INPUT")?);
    let output = PathBuf::from(std::env::var("ROAD_ROUTE_OUTPUT")?);
    let ticks = std::env::var("ROAD_ROUTE_TICKS")?.parse::<u32>()?;
    let world = World::decode(&Savegame::decode(
        &std::fs::read(input)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let before = format!("{runtime:?}");
    let before_world = runtime.saved_json()?;
    let live_before = runtime.transport_state_json();
    std::fs::create_dir_all(&output)?;
    // When: the entire native-observed route horizon advances in one transaction.
    let result = runtime.advance_world(ticks);
    // Then: refusals preserve committed saved/cache/ownership state. Host workers may
    // independently finish; their raw before/after observations remain visible.
    //
    // the required route still fails RED until the whole horizon is supported.
    let unchanged = before == format!("{runtime:?}");
    let live_after = runtime.transport_state_json();
    let description = match &result {
        Ok(report) => format!("Ok({} ticks)", report.ticks.len()),
        Err(error) => format!("Err({error:?})"),
    };
    std::fs::write(
        output.join("result.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "ticks": ticks, "result": description,
            "committed_runtime_unchanged": unchanged,
            "entire_runtime_unchanged": unchanged && live_before == live_after,
            "live_transport_before": live_before, "live_transport_after": live_after,
            "saved_world_unchanged": before_world == runtime.saved_json()?
        }))?,
    )?;
    if result.is_err() {
        assert!(unchanged, "rejected round trip published tentative state");
    }
    assert!(
        result.is_ok(),
        "native round trip remains unsupported: {result:?}"
    );
    std::fs::write(
        output.join("final.world.json"),
        serde_json::to_vec(&runtime.saved_json()?)?,
    )?;
    std::fs::write(
        output.join("final.sav"),
        runtime.to_savegame()?.encode(Compression::None)?,
    )?;
    std::fs::write(
        output.join("final.derived.json"),
        serde_json::to_vec(runtime.world().derived())?,
    )?;
    std::fs::write(
        output.join("final.runtime.json"),
        serde_json::to_vec(&serde_json::json!({
            "road": runtime.road_caches().values().collect::<Vec<_>>(),
            "station_geometry": runtime.road_station_geometry(),
            "station_infrastructure": runtime.station_infrastructure(),
            "station_pool": runtime.station_pool(), "road_stop_pool": runtime.road_stop_pool(),
            "transport": runtime.transport_state_json(),
            "house_caches": runtime.town_house_caches(),
            "spatial": runtime.single_road_tile_occupancy()?, "orders": runtime.order_state_json()?
        }))?,
    )?;
    Ok(())
}
