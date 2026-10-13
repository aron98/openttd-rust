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
    assert_eq!(
        runtime.news_queue().len(),
        owned_news_queue(&expected)?,
        "{label}: process-owned news queue length"
    );
    Ok(())
}

/// One probed native unprofitable-branch event from the regenerated year trace.
struct Probe {
    sequence: u64,
    phase: String,
    vehicle: u64,
    extra: Value,
    news_queue: u64,
    economy_age: i64,
    profit_this_year: i64,
}

fn unprofitable_probes(native: &Path) -> Result<Vec<Probe>> {
    let text = fs::read_to_string(native.join("year.jsonl"))?;
    let mut probes = Vec::new();
    for line in text.lines() {
        let row: Value = serde_json::from_str(line)?;
        let phase = row
            .get("phase")
            .and_then(Value::as_str)
            .ok_or("native probe phase")?
            .to_string();
        if phase != "unprofitable_news" && phase != "unprofitable_ai" {
            continue;
        }
        let snapshot = row.get("snapshot").ok_or("native probe snapshot")?;
        let vehicle = snapshot
            .get("vehicles")
            .and_then(Value::as_array)
            .ok_or("native probe vehicles")?
            .iter()
            .find(|entry| entry.get("id").is_some_and(|id| *id == 0))
            .ok_or("native probe vehicle 0")?;
        probes.push(Probe {
            sequence: row
                .get("sequence")
                .and_then(Value::as_u64)
                .ok_or("native probe sequence")?,
            vehicle: row
                .get("vehicle")
                .and_then(Value::as_u64)
                .ok_or("native probe vehicle")?,
            news_queue: snapshot
                .get("news_queue")
                .and_then(Value::as_u64)
                .ok_or("native probe queue")?,
            economy_age: vehicle
                .get("economy_age")
                .and_then(Value::as_i64)
                .ok_or("native probe age")?,
            profit_this_year: vehicle
                .get("profit_this_year")
                .and_then(Value::as_i64)
                .ok_or("native probe profit")?,
            extra: row.get("extra").cloned().ok_or("native probe context")?,
            phase,
        });
    }
    Ok(probes)
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
#[ignore = "requires regenerated original warning-boundary initial/prefix/final snapshots"]
fn original_unprofitable_warning_matches_native_news_parity() -> Result {
    let native = PathBuf::from(std::env::var("INDUSTRY_RECIPIENT_NATIVE")?);
    let output = PathBuf::from(std::env::var("VEHICLE_YEAR_OUTPUT")?);
    let calls: u32 = std::env::var("VEHICLE_YEAR_CALLS")?.parse()?;
    fs::create_dir(&output)?;
    let world = World::decode(&Savegame::decode(
        &fs::read(native.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    // Given the regenerated capture, the positive prefix stops one call short of the warning.
    let mut positive = SimulationRuntime::restore_vanilla(world.clone())?;
    native_views::compare(&positive, &output, "initial")?;
    compare_groups(&positive, &native, &output, "initial")?;
    assert_eq!(
        positive
            .advance_world(calls.checked_sub(1).ok_or("empty prefix")?)?
            .ticks
            .len(),
        usize::try_from(calls - 1)?
    );
    native_views::compare(&positive, &output, "prefix")?;
    compare_groups(&positive, &native, &output, "prefix")?;
    assert!(positive.news_queue().is_empty());
    // When the full request executes the implemented native unprofitable branch.
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let before = snapshot(&runtime)?;
    let result = runtime.advance_world(calls);
    let success = result.is_ok();
    let after = snapshot(&runtime)?;
    fs::write(
        output.join("result.json"),
        serde_json::to_vec_pretty(&json!({
            "success":success,"calls":calls,"unchanged":before==after,
            "native_warning_context_only":false,"news_queue_parity":success,
        }))?,
    )?;
    assert_eq!(result?.ticks.len(), usize::try_from(calls)?);
    assert_ne!(before, after);
    native_views::compare(&runtime, &output, "final")?;
    compare_groups(&runtime, &native, &output, "final")?;
    // Then the probes show exactly one news item and one human-owner AI drop.
    let probes = unprofitable_probes(&native)?;
    assert_eq!(
        probes.len(),
        2,
        "exactly one news probe and one AI drop probe"
    );
    let news = probes
        .iter()
        .find(|probe| probe.phase == "unprofitable_news")
        .ok_or("unprofitable_news probe")?;
    let ai = probes
        .iter()
        .find(|probe| probe.phase == "unprofitable_ai")
        .ok_or("unprofitable_ai probe")?;
    assert_eq!(news.vehicle, 0);
    assert_eq!(news.economy_age, 1094);
    assert_eq!(news.profit_this_year, -287_985);
    assert_eq!(
        news.extra,
        json!({"company": 0, "string": 761, "profit": -1125})
    );
    assert_eq!(news.news_queue, 0, "the news probe fires before the insert");
    assert_eq!(ai.vehicle, 0);
    assert_eq!(ai.extra, json!({"company": 0, "is_ai": false}));
    assert_eq!(ai.news_queue, 1, "the AI drop probe fires after the insert");
    assert!(
        news.sequence < ai.sequence,
        "the news item is inserted before the AI event drop"
    );
    // And the Rust process-owned queue matches the native probe exactly.
    assert_eq!(
        serde_json::to_value(runtime.news_queue())?,
        json!({"vehicle_income_warn": true, "items":
            [{"company": 0, "vehicle": 0, "string": 761, "profit": -1125}]}),
        "process-owned news queue parity with the native probe"
    );
    fs::write(
        output.join("final.sav"),
        runtime.to_savegame()?.encode(Compression::Zlib)?,
    )?;
    Ok(())
}
