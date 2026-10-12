//! Same-input original whole-state and owned group-profit continuation proof.
use ottd_save::{Compression, Savegame, world::World};
use ottd_sim::runtime::SimulationRuntime;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
include!("vehicle_year_support/groups.rs");
#[path = "industry_recipient_support/native_views.rs"]
mod native_views;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn snapshot(runtime: &SimulationRuntime) -> Result<Value> {
    Ok(json!({
        "world":runtime.saved_json()?, "derived":runtime.world().derived(),
        "complete_runtime":format!("{runtime:?}"), "group_profits":runtime.group_profit_caches(),
        "content":runtime.content(), "transport":runtime.transport_state_json(),
    }))
}
fn compare_groups(
    runtime: &SimulationRuntime,
    native: &Path,
    output: &Path,
    label: &str,
) -> Result {
    let expected: Value =
        serde_json::from_slice(&fs::read(native.join(format!("{label}.groups.json")))?)?;
    let actual = serde_json::to_value(runtime.group_profit_caches())?;
    fs::write(
        output.join(format!("{label}.group-profits.json")),
        serde_json::to_vec_pretty(&actual)?,
    )?;
    assert_eq!(
        actual,
        owned_native(&expected)?,
        "{label}: all three owned group-profit fields"
    );
    Ok(())
}
#[test]
#[ignore = "requires unchanged original initial/final saves, groups and all nine cache coordinates"]
fn original_vehicle_year_advance_matches_complete_state() -> Result {
    let native = PathBuf::from(std::env::var("INDUSTRY_RECIPIENT_NATIVE")?);
    let output = PathBuf::from(std::env::var("VEHICLE_YEAR_OUTPUT")?);
    let calls: u32 = std::env::var("VEHICLE_YEAR_CALLS")?.parse()?;
    fs::create_dir(&output)?;
    let mut runtime = SimulationRuntime::restore_vanilla(World::decode(&Savegame::decode(
        &fs::read(native.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)?;
    let before = snapshot(&runtime)?;
    fs::write(output.join("before.json"), serde_json::to_vec(&before)?)?;
    native_views::compare(&runtime, &output, "initial")?;
    compare_groups(&runtime, &native, &output, "initial")?;
    let result = runtime.advance_world(calls);
    let after = snapshot(&runtime)?;
    fs::write(output.join("after.json"), serde_json::to_vec(&after)?)?;
    fs::write(
        output.join("result.json"),
        serde_json::to_vec_pretty(&json!({
            "success":result.is_ok(),"calls":calls,"error":result.as_ref().err().map(ToString::to_string),"unchanged":before==after,
        }))?,
    )?;
    assert_eq!(result?.ticks.len(), usize::try_from(calls)?);
    native_views::compare(&runtime, &output, "final")?;
    compare_groups(&runtime, &native, &output, "final")?;
    fs::write(
        output.join("final.sav"),
        runtime.to_savegame()?.encode(Compression::Zlib)?,
    )?;
    Ok(())
}
