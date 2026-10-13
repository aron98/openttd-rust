//! Synthetic service predicates plus genuine, unmodified year-boundary continuation.
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
#[ignore = "requires NATURAL_YEAR_NATIVE and NATURAL_OUTPUT; historical refusal selector retained"]
fn genuine_positive_cargo_prefix_then_vehicle_year_refusal_is_atomic() -> Result {
    let native = PathBuf::from(std::env::var("NATURAL_YEAR_NATIVE")?);
    let output = PathBuf::from(std::env::var("NATURAL_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    // Given the unchanged original passenger input and its observed prefix/year snapshots.
    let mut runtime = load(&native.join("initial.sav"))?;
    super::year_native::compare(&runtime, &native, "initial")?;
    assert_eq!(runtime.advance_world(18817)?.ticks.len(), 18817);
    super::year_native::compare(&runtime, &native, "prefix")?;
    // When the next call crosses the actual original year boundary.
    assert_eq!(runtime.advance_world(1)?.ticks.len(), 1);
    // Then the formerly refused call matches every explicitly compared native coordinate.
    super::year_native::compare(&runtime, &native, "final")?;
    std::fs::write(
        output.join("PASS.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "synthetic": false, "positive_prefix":18817, "year_call":18818,
            "historical_selector":"genuine_positive_cargo_prefix_then_vehicle_year_refusal_is_atomic",
            "year_supported":true, "whole_world_derived_nine_views_three_group_fields":true,
            "rollback_credit":false
        }))?,
    )?;
    Ok(())
}
