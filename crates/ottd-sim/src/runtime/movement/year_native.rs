//! Exact native coordinate comparison for an unchanged wood producer continuation.
use crate::runtime::SimulationRuntime;
use serde_json::{Value, json};
use std::{fs, path::Path};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
include!("../../../tests/vehicle_year_support/groups.rs");
pub(super) fn compare(runtime: &SimulationRuntime, native: &Path, label: &str) -> Result {
    for (view, actual) in [
        ("world", runtime.saved_json()?),
        ("derived", serde_json::to_value(runtime.world().derived())?),
    ] {
        let name = format!("{label}.{view}.json");
        let expected: Value = serde_json::from_slice(&fs::read(native.join(name))?)?;
        assert_eq!(actual, expected, "{label}/{view}");
    }
    let actual = json!({
        "house_caches": runtime.town_house_caches(),
        "industry_caches": runtime.industry_caches(),
        "transport": runtime.transport_state_json(),
        "runtime_road": {"road": runtime.road_caches().values().collect::<Vec<_>>()},
        "stations": {
            "orders": runtime.order_state_json()?,
            "geometry": runtime.road_station_geometry(),
            "infrastructure": runtime.station_infrastructure(),
            "station_pool": runtime.station_pool(),
            "road_stop_pool": runtime.road_stop_pool(),
        },
    });
    let name = format!("{label}.movement.json");
    let expected: Value = serde_json::from_slice(&fs::read(native.join(name))?)?;
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
            actual.pointer(field).ok_or("missing actual coordinate")?,
            expected.pointer(field).ok_or("missing native coordinate")?,
            "{label}{field}",
        );
    }
    let expected: Value =
        serde_json::from_slice(&fs::read(native.join(format!("{label}.groups.json")))?)?;
    assert_eq!(
        serde_json::to_value(runtime.group_profit_caches())?,
        owned_native(&expected)?
    );
    Ok(())
}
