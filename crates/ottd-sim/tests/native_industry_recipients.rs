//! Success-expecting recipient REDs use unchanged original simulation-produced saves.
use ottd_save::{Compression, Savegame, world::World};
use ottd_sim::runtime::SimulationRuntime;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn snapshot(runtime: &SimulationRuntime) -> Result<Value> {
    Ok(json!({
        "world": runtime.saved_json()?,
        "derived": runtime.world().derived(),
        "content": runtime.content(),
        "road": runtime.road_caches(),
        "town": runtime.town_house_caches(),
        "industry": runtime.industry_caches(),
        "transport": runtime.transport_state_json(),
        "stations": runtime.station_pool(),
        "stops": runtime.road_stop_pool(),
        "geometry": runtime.road_station_geometry(),
        "station_infrastructure": runtime.station_infrastructure(),
        "depot": runtime.depot_pool(),
        "road_infrastructure": runtime.road_infrastructure().iter()
            .map(|(owner, types)| (owner, types.as_slice())).collect::<std::collections::BTreeMap<_, _>>(),
    }))
}

#[test]
#[ignore = "requires the retained original wood recipient corpus"]
fn original_industry_recipients_advance_matches_complete_world() -> Result {
    let source = PathBuf::from(std::env::var("INDUSTRY_RECIPIENT_INPUT")?);
    let expected = PathBuf::from(std::env::var("INDUSTRY_RECIPIENT_EXPECTED")?);
    let output = PathBuf::from(std::env::var("INDUSTRY_RECIPIENT_OUTPUT")?);
    let calls: u32 = std::env::var("INDUSTRY_RECIPIENT_CALLS")?.parse()?;
    fs::create_dir(&output)?;
    let world = World::decode(&Savegame::decode(
        &fs::read(source)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let restoration = SimulationRuntime::restore_vanilla(world);
    fs::write(
        output.join("restore.json"),
        serde_json::to_vec_pretty(&json!({
            "success": restoration.is_ok(), "error": restoration.as_ref().err().map(ToString::to_string),
        }))?,
    )?;
    let mut runtime = restoration?;
    let before = snapshot(&runtime)?;
    fs::write(output.join("before.json"), serde_json::to_vec(&before)?)?;
    let result = runtime.advance_world(calls);
    let after = snapshot(&runtime)?;
    fs::write(output.join("after.json"), serde_json::to_vec(&after)?)?;
    fs::write(
        output.join("result.json"),
        serde_json::to_vec_pretty(&json!({
            "requested_calls": calls, "success": result.is_ok(),
            "error": result.as_ref().err().map(ToString::to_string), "unchanged": before == after,
        }))?,
    )?;
    let _advanced = result?;
    let native = World::decode(&Savegame::decode(
        &fs::read(expected)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    assert_eq!(
        runtime.saved_json()?,
        native.saved_json()?,
        "complete original World"
    );
    assert_eq!(
        runtime.world().derived(),
        native.derived(),
        "complete derived state"
    );
    fs::write(
        output.join("final.sav"),
        runtime.to_savegame()?.encode(Compression::Zlib)?,
    )?;
    fs::write(
        output.join("PASS.json"),
        serde_json::to_vec_pretty(&json!({"calls": calls}))?,
    )?;
    Ok(())
}
