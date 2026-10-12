//! Whole-interval native comparison including the actual empty and populated caches.
use ottd_save::{Compression, Savegame, world::World};
use ottd_sim::runtime::SimulationRuntime;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn read(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

fn capture(runtime: &SimulationRuntime, native: &Path, output: &Path, label: &str) -> Result {
    for (view, actual) in [
        ("world", runtime.saved_json()?),
        ("derived", serde_json::to_value(runtime.world().derived())?),
    ] {
        let name = format!("{label}.{view}.json");
        std::fs::write(output.join(&name), serde_json::to_vec(&actual)?)?;
        assert_eq!(actual, read(&native.join(name))?, "{label}/{view}");
    }
    let actual = json!({
        "house_caches": runtime.town_house_caches(),
        "industry_caches": runtime.industry_caches(),
        "transport": runtime.transport_state_json(),
        "runtime_road": {
            "road": runtime.road_caches().values().collect::<Vec<_>>(),
            "pool": runtime.vehicle_pool(),
            "units": runtime.road_unit_numbers()?,
        },
        "stations": {
            "orders": runtime.order_state_json()?,
            "geometry": runtime.road_station_geometry(),
            "infrastructure": runtime.station_infrastructure(),
            "station_pool": runtime.station_pool(),
            "road_stop_pool": runtime.road_stop_pool(),
        },
    });
    std::fs::write(
        output.join(format!("{label}.movement.json")),
        serde_json::to_vec(&actual)?,
    )?;
    let expected = read(&native.join(format!("{label}.movement.json")))?;
    for field in [
        "/house_caches",
        "/industry_caches",
        "/transport",
        "/runtime_road/road",
        "/runtime_road/pool",
        "/stations/orders",
        "/stations/geometry",
        "/stations/infrastructure",
        "/stations/station_pool",
        "/stations/road_stop_pool",
    ] {
        assert_eq!(
            actual.pointer(field).ok_or("missing actual field")?,
            expected.pointer(field).ok_or("missing native field")?,
            "{label}{field}",
        );
    }
    // The native units count field is not a restored runtime field; compare only
    // the genuinely restored per-company unit-number generator state.
    let units = expected
        .pointer("/runtime_road/units")
        .and_then(Value::as_array)
        .ok_or("missing native units")?
        .iter()
        .map(|row| {
            Ok(json!({
                "company": row.get("company").ok_or("missing native unit company")?,
                "next": row.get("next").ok_or("missing native unit next")?,
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    assert_eq!(
        actual
            .pointer("/runtime_road/units")
            .ok_or("missing actual field")?,
        &Value::Array(units),
        "{label}/runtime_road/units",
    );
    Ok(())
}

#[test]
#[ignore = "requires current original whole-runtime observations on an unchanged saved input"]
fn whole_interval_matches_original_saved_derived_and_actual_caches() -> Result {
    let native = PathBuf::from(std::env::var("OTTD_WHOLE_NATIVE")?);
    let output = PathBuf::from(std::env::var("OTTD_WHOLE_OUTPUT")?);
    let calls: u32 = std::env::var("OTTD_WHOLE_CALLS")?.parse()?;
    std::fs::create_dir_all(&output)?;
    let world = World::decode(&Savegame::decode(
        &std::fs::read(native.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    capture(&runtime, &native, &output, "initial")?;
    runtime.advance_world(calls)?;
    capture(&runtime, &native, &output, "final")?;
    std::fs::write(
        output.join("final.sav"),
        runtime.to_savegame()?.encode(Compression::None)?,
    )?;
    std::fs::write(
        output.join("PASS.json"),
        serde_json::to_vec(&json!({
            "calls": calls, "complete_saved_derived_and_actual_cache_fields": true,
            "native_directory": native,
        }))?,
    )?;
    Ok(())
}
