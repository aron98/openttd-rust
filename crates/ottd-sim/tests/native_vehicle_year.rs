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

#[test]
#[ignore = "requires actual original age730/before731/final snapshots and all native coordinates"]
fn original_age731_split_calls_preserve_exact_threshold() -> Result {
    let native = PathBuf::from(std::env::var("INDUSTRY_RECIPIENT_NATIVE")?);
    let output = PathBuf::from(std::env::var("VEHICLE_YEAR_OUTPUT")?);
    fs::create_dir(&output)?;
    let mut runtime = SimulationRuntime::restore_vanilla(World::decode(&Savegame::decode(
        &fs::read(native.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)?;
    native_views::compare(&runtime, &output, "initial")?;
    compare_groups(&runtime, &native, &output, "initial")?;
    let before = serde_json::to_value(runtime.group_profit_caches())?;
    assert_eq!(runtime.advance_world(73)?.ticks.len(), 73);
    native_views::compare(&runtime, &output, "before731")?;
    compare_groups(&runtime, &native, &output, "before731")?;
    assert_eq!(before, serde_json::to_value(runtime.group_profit_caches())?);
    assert_eq!(runtime.advance_world(1)?.ticks.len(), 1);
    native_views::compare(&runtime, &output, "final")?;
    compare_groups(&runtime, &native, &output, "final")?;
    assert_ne!(before, serde_json::to_value(runtime.group_profit_caches())?);
    fs::write(
        output.join("final.sav"),
        runtime.to_savegame()?.encode(Compression::Zlib)?,
    )?;
    Ok(())
}

#[test]
#[ignore = "requires actual original warning-boundary initial/final snapshots"]
fn original_warning_context_remains_refused_without_committed_changes() -> Result {
    let native = PathBuf::from(std::env::var("INDUSTRY_RECIPIENT_NATIVE")?);
    let output = PathBuf::from(std::env::var("VEHICLE_YEAR_OUTPUT")?);
    fs::create_dir(&output)?;
    let mut runtime = SimulationRuntime::restore_vanilla(World::decode(&Savegame::decode(
        &fs::read(native.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)?;
    native_views::compare(&runtime, &output, "initial")?;
    compare_groups(&runtime, &native, &output, "initial")?;
    let original_final = World::decode(&Savegame::decode(
        &fs::read(native.join("final.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    assert_ne!(runtime.saved_json()?, original_final.saved_json()?);
    let before = snapshot(&runtime)?;
    let result = runtime.advance_world(1);
    let after = snapshot(&runtime)?;
    fs::write(
        output.join("result.json"),
        serde_json::to_vec_pretty(&json!({
            "error":result.as_ref().err().map(ToString::to_string),"unchanged":before==after,
            "native_warning_context_only":true,"news_queue_parity":false
        }))?,
    )?;
    let error = result.expect_err("native warning/news/AI context remains required");
    assert!(
        error
            .to_string()
            .contains("call 0: unprofitable warning/news and AI event context"),
        "{error}"
    );
    assert_eq!(before, after);
    for (label, value) in [("before", before), ("after", after)] {
        fs::write(
            output.join(format!("{label}.json")),
            serde_json::to_vec(&value)?,
        )?;
    }
    Ok(())
}
