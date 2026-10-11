//! Synthetic service predicates plus genuine, unmodified year-boundary rollback.
use super::{State, VehicleId, number, service};
use crate::runtime::SimulationRuntime;
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use std::path::{Path, PathBuf};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn load(path: &Path) -> Result<SimulationRuntime> {
    Ok(SimulationRuntime::restore_vanilla(World::decode(
        &Savegame::decode(&std::fs::read(path)?, ottd_save::DEFAULT_MAX_BYTES)?,
    )?)?)
}
#[test]
#[ignore = "requires genuine NO_SERVICE_INPUT and NO_SERVICE_OUTPUT; explicitly synthetic predicate controls"]
fn service_interval_and_replacement_list_predicates() -> Result {
    let input = PathBuf::from(std::env::var("NO_SERVICE_INPUT")?);
    let output = PathBuf::from(std::env::var("NO_SERVICE_OUTPUT")?);
    let mut results = Vec::new();
    for (offset, disabled, breakdowns, renew, renew_enabled, refusal) in [
        (0, 0, 1, 1, 1, false),
        (1, 1, 0, 0, 1, false),
        (1, 1, 0, 0, 0, false),
        (1, 0, 0, 0, 0, true),
        (1, 1, 1, 0, 0, true),
        (1, 1, 0, 1, 0, true),
    ] {
        let mut runtime = load(&input)?;
        let before = format!("{runtime:?}");
        {
            let industries = crate::runtime::IndustryCaches::restore(&runtime.world)?;
            let mut state = State::load(&mut runtime.world, None, industries);
            let date = number(&state, VehicleId::new(0), false, "date_of_last_service")?
                + number(&state, VehicleId::new(0), false, "service_interval")?
                + offset;
            state.set_number(b"DATE", 0, "economy_date", date)?;
            state.set_number(b"PATS", 0, "order.no_servicing_if_no_breakdowns", disabled)?;
            state.set_number(b"PATS", 0, "difficulty.vehicle_breakdowns", breakdowns)?;
            for (field, value) in [
                ("engine_renew_list", renew),
                ("settings.engine_renew", renew_enabled),
            ] {
                state.apply(WorldEdit::Field {
                    chunk: *b"PLYR",
                    record: 0,
                    path: vec![
                        PathElement::Field("settings".into()),
                        PathElement::Index(0),
                        PathElement::Field(field.into()),
                    ],
                    value: WireValue::Unsigned(value),
                })?;
            }
            assert_eq!(service::check(&state, VehicleId::new(0)).is_err(), refusal);
        }
        assert_eq!(format!("{runtime:?}"), before);
        results.push(serde_json::json!({"synthetic":true,"days_after_interval":offset,"no_service_without_breakdowns":disabled,
            "breakdowns":breakdowns,"renewal_list_pointer":renew,"renewal_enabled":renew_enabled,"refused":refusal,"committed_unchanged":true}));
    }
    std::fs::create_dir_all(&output)?;
    std::fs::write(
        output.join("PASS.json"),
        serde_json::to_vec_pretty(&results)?,
    )?;
    Ok(())
}
#[test]
#[ignore = "requires unchanged NATURAL_INPUT, NATURAL_PREFIX, NATURAL_BOUNDARY and NATURAL_OUTPUT"]
fn genuine_positive_cargo_prefix_then_vehicle_year_refusal_is_atomic() -> Result {
    let input = PathBuf::from(std::env::var("NATURAL_INPUT")?);
    let prefix = PathBuf::from(std::env::var("NATURAL_PREFIX")?);
    let boundary = PathBuf::from(std::env::var("NATURAL_BOUNDARY")?);
    let output = PathBuf::from(std::env::var("NATURAL_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    let mut positive = load(&input)?;
    assert_eq!(positive.advance_world(18817)?.ticks.len(), 18817);
    let native: serde_json::Value =
        serde_json::from_slice(&std::fs::read(prefix.join("native/final.world.json"))?)?;
    assert_eq!(positive.saved_json()?, native);
    let native_derived: serde_json::Value =
        serde_json::from_slice(&std::fs::read(prefix.join("native/final.derived.json"))?)?;
    assert_eq!(
        serde_json::to_value(positive.world.derived())?,
        native_derived
    );
    let live: serde_json::Value =
        serde_json::from_slice(&std::fs::read(prefix.join("native-last-tick.json"))?)?;
    assert_eq!(
        Some(&positive.transport_state_json()),
        live.pointer("/movement/transport")
    );
    assert_eq!(
        Some(&serde_json::to_value(positive.town_house_caches())?),
        live.pointer("/movement/house_caches")
    );
    assert_eq!(
        Some(&serde_json::to_value(
            positive.road_caches().values().collect::<Vec<_>>()
        )?),
        live.pointer("/movement/runtime_road/road")
    );
    let before_year: serde_json::Value = serde_json::from_slice(&std::fs::read(boundary)?)?;
    assert_eq!(
        before_year
            .pointer("/runtime/economy_year")
            .and_then(serde_json::Value::as_u64),
        Some(2101)
    );
    let mut runtime = load(&input)?;
    let before = snapshot(&runtime)?;
    let error = runtime
        .advance_world(18818)
        .expect_err("genuine next year remains required");
    assert!(format!("{error:?}").contains("call 18817: vehicle month/year callbacks"));
    let after = snapshot(&runtime)?;
    assert_eq!(before, after);
    std::fs::write(output.join("before.json"), serde_json::to_vec(&before)?)?;
    std::fs::write(output.join("after.json"), serde_json::to_vec(&after)?)?;
    std::fs::write(
        output.join("PASS.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "synthetic":false,"same_input_shorter_success":18817,"whole_requested":18818,
            "original_valid_year":2101,"actual_refusal":format!("{error:?}"),"complete_committed_world_cache_unchanged":true,
            "parity_credit":"positive prefix only; vehicle-year consumer remains required"
        }))?,
    )?;
    Ok(())
}
fn snapshot(runtime: &SimulationRuntime) -> Result<serde_json::Value> {
    Ok(
        serde_json::json!({"complete_debug":format!("{runtime:?}"),"world":runtime.saved_json()?,
        "derived":runtime.world.derived(),"transport":runtime.transport_state_json(),
        "house_caches":runtime.town_house_caches(),"road_caches":runtime.road_caches(),
        "station_geometry":runtime.road_station_geometry(),"station_infrastructure":runtime.station_infrastructure(),
        "station_pool":runtime.station_pool(),"road_stop_pool":runtime.road_stop_pool(),
        "spatial":runtime.single_road_tile_occupancy()?,"orders":runtime.order_state_json()?}),
    )
}
