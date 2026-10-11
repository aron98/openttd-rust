//! A genuine completed native wood prefix precedes an unsupported year with a retained worker owner.
use crate::runtime::{SimulationRuntime, job_executor::Backend};
use ottd_save::{Savegame, world::World};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn read(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

fn load(path: &Path) -> Result<SimulationRuntime> {
    Ok(SimulationRuntime::restore_vanilla(World::decode(
        &Savegame::decode(&std::fs::read(path)?, ottd_save::DEFAULT_MAX_BYTES)?,
    )?)?)
}

fn views(runtime: &SimulationRuntime) -> Result<Value> {
    Ok(json!({
        "house_caches": runtime.town_house_caches(),
        "industry_caches": runtime.industry_caches(),
        "transport": runtime.transport_state_json(),
        "runtime_road": {"road": runtime.road_caches().values().collect::<Vec<_>>()},
        "stations": {
            "orders": runtime.order_state_json()?, "geometry": runtime.road_station_geometry(),
            "infrastructure": runtime.station_infrastructure(), "station_pool": runtime.station_pool(),
            "road_stop_pool": runtime.road_stop_pool(),
        },
    }))
}

fn committed(runtime: &SimulationRuntime) -> Result<Value> {
    Ok(
        json!({"complete_runtime":format!("{runtime:?}"),"world":runtime.saved_json()?,
        "derived":runtime.world.derived(),"views":views(runtime)?,
        "content":runtime.content(),"depot":runtime.depot_pool(),
        "road_infrastructure":runtime.road_infrastructure().iter()
            .map(|(owner, types)| (owner, types.as_slice())).collect::<std::collections::BTreeMap<_,_>>(),
        "spatial":runtime.single_road_tile_occupancy()?}),
    )
}

#[test]
#[ignore = "requires WOOD_ROLLBACK_NATIVE/OUTPUT; actual native completion branch, pending branch remains separately required"]
fn genuine_wood_handoff_loaded_prefix_then_year_refusal_preserves_worker_owner() -> Result {
    let native = PathBuf::from(std::env::var("WOOD_ROLLBACK_NATIVE")?);
    let output = PathBuf::from(std::env::var("WOOD_ROLLBACK_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    // Given actual native tick3080 with a real saved in-progress job and no wood packet yet.
    let input = native.join("initial.sav");
    let mut positive = load(&input)?;
    let initial = positive.saved_json()?;
    assert!(
        initial
            .pointer("/chunks/CAPA/records")
            .ok_or("packet table")?
            .as_object()
            .ok_or("packet table")?
            .is_empty()
    );
    assert_eq!(positive.advance_world(23_929)?.ticks.len(), 23_929);
    assert_ne!(initial, positive.saved_json()?);
    compare_prefix(&positive, &native, &output)?;
    let boundary = read(&native.join("final.runtime.json"))?;
    assert_eq!(
        boundary.pointer("/after_save/calendar_year"),
        Some(&json!(2101))
    );
    assert_eq!(boundary.pointer("/after_save/tick"), Some(&json!(27010)));
    let mut runtime = load(&input)?;
    let owner = Arc::clone(
        runtime
            .transport
            .workers
            .get(&0)
            .ok_or("real initial worker")?,
    );
    let before = committed(&runtime)?;
    // When the same input executes the positively mutating prefix and reaches the actual year boundary.
    let error = runtime
        .advance_world(23_930)
        .expect_err("vehicle year remains unsupported");
    assert!(
        error
            .to_string()
            .contains("call 23929: vehicle month/year callbacks"),
        "{error}"
    );
    // Then every committed owned cache and the original asynchronous owner survive candidate cleanup.
    let after = committed(&runtime)?;
    assert_eq!(before, after);
    assert!(Arc::ptr_eq(
        &owner,
        runtime.transport.workers.get(&0).ok_or("lost worker")?
    ));
    assert_eq!(Arc::strong_count(&owner), 2);
    assert_eq!(owner.backend(), Backend::Threaded);
    assert!(owner.executing_thread().is_some());
    assert_ne!(owner.executing_thread(), Some(std::thread::current().id()));
    assert!(owner.poll()?.is_some());
    for (label, value) in [("before", before), ("after", after)] {
        std::fs::write(
            output.join(format!("{label}.json")),
            serde_json::to_vec(&value)?,
        )?;
    }
    std::fs::write(
        output.join("PASS.json"),
        serde_json::to_vec_pretty(&json!({
            "synthetic":false,"input_tick":3080,"positive_calls":23929,"whole_calls":23930,
            "actual_original_year":2101,"error":error.to_string(),"full_committed_state_unchanged":true,
            "preexisting_worker_owner_preserved":true,"owners_after":2,"no_prejoin_or_forced_completion":true,
            "scope":"observed completed native branch only; checkpoint-only pending branch remains required",
        }))?,
    )?;
    Ok(())
}

fn compare_prefix(positive: &SimulationRuntime, native: &Path, output: &Path) -> Result {
    let world = positive.saved_json()?;
    assert_eq!(world, read(&native.join("prefix.world.json"))?);
    let derived = serde_json::to_value(positive.world.derived())?;
    assert_eq!(derived, read(&native.join("prefix.derived.json"))?);
    let actual = views(positive)?;
    let expected = read(&native.join("prefix.movement.json"))?;
    for field in [
        "/house_caches",
        "/industry_caches",
        "/transport",
        "/runtime_road/road",
        "/stations/orders",
        "/stations/geometry",
        "/stations/infrastructure",
        "/stations/station_pool",
        "/stations/road_stop_pool",
    ] {
        assert_eq!(
            actual.pointer(field).ok_or("actual coordinate")?,
            expected.pointer(field).ok_or("native coordinate")?,
            "{field}"
        );
    }
    assert_eq!(
        positive
            .transport
            .cargo
            .vehicles
            .get(&0)
            .ok_or("wood truck")?
            .count,
        24
    );
    for (label, value) in [("world", world), ("derived", derived), ("movement", actual)] {
        std::fs::write(
            output.join(format!("positive-{label}.json")),
            serde_json::to_vec(&value)?,
        )?;
    }
    Ok(())
}
