//! Original company-quarter comparisons include all 150 process-owned score slots.
use ottd_save::{Compression, Savegame, world::World};
use ottd_sim::runtime::SimulationRuntime;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
include!("vehicle_year_support/groups.rs");
include!("company_rating_support/ratings.rs");
#[path = "industry_recipient_support/native_views.rs"]
mod native_views;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn snapshot(runtime: &SimulationRuntime) -> Result<Value> {
    Ok(
        json!({"world":runtime.saved_json()?, "derived":runtime.world().derived(),
        "runtime":format!("{runtime:?}"), "ratings":runtime.company_rating_caches(),
        "groups":runtime.group_profit_caches(), "transport":runtime.transport_state_json()}),
    )
}
fn compare(runtime: &SimulationRuntime, output: &Path, label: &str) -> Result {
    native_views::compare(runtime, output, label)?;
    let native = PathBuf::from(std::env::var("INDUSTRY_RECIPIENT_NATIVE")?);
    let groups: Value =
        serde_json::from_slice(&fs::read(native.join(format!("{label}.groups.json")))?)?;
    assert_eq!(
        serde_json::to_value(runtime.group_profit_caches())?,
        owned_native(&groups)?
    );
    fs::write(
        output.join(format!("{label}.ratings.json")),
        serde_json::to_vec(runtime.company_rating_caches())?,
    )?;
    compare_raw_ratings(runtime, &native, label)?;
    Ok(())
}

#[test]
#[ignore = "requires original input/final World, all cache coordinates and company-rating snapshots"]
fn original_quarter_matches_complete_world_and_raw_rating_cache() -> Result {
    let native = PathBuf::from(std::env::var("INDUSTRY_RECIPIENT_NATIVE")?);
    let output = PathBuf::from(std::env::var("COMPANY_RATING_OUTPUT")?);
    let calls: u32 = std::env::var("COMPANY_RATING_CALLS")?.parse()?;
    fs::create_dir(&output)?;
    let mut runtime = SimulationRuntime::restore_vanilla(World::decode(&Savegame::decode(
        &fs::read(native.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)?;
    compare(&runtime, &output, "initial")?;
    let before = snapshot(&runtime)?;
    fs::write(output.join("before.json"), serde_json::to_vec(&before)?)?;
    let result = runtime.advance_world(calls);
    let after = snapshot(&runtime)?;
    fs::write(output.join("after.json"), serde_json::to_vec(&after)?)?;
    fs::write(
        output.join("result.json"),
        serde_json::to_vec_pretty(&json!({
            "calls":calls,"success":result.is_ok(),"error":result.as_ref().err().map(ToString::to_string),"unchanged":before==after,
        }))?,
    )?;
    assert!(
        result.is_ok(),
        "genuine original-success quarter: {result:?}"
    );
    compare(&runtime, &output, "final")?;
    fs::write(
        output.join("final.sav"),
        runtime.to_savegame()?.encode(Compression::Zlib)?,
    )?;
    Ok(())
}
