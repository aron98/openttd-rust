//! Original-valid automatic industry policy remains an explicit transactional boundary.
use ottd_save::{Savegame, world::World};
use ottd_sim::WorldTickError;
use std::{fs::File, path::PathBuf};
#[path = "industry_recipient_support/native_views.rs"]
mod native_views;
include!("disaster_support/mod.rs");
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "requires successful original density command and natural builder callback"]
fn automatic_manager_refuses_without_publishing_any_runtime_effect() -> Result {
    let native = PathBuf::from(std::env::var("OTTD_INDUSTRY_AUTOMATIC_DIR")?);
    let input = World::decode(&Savegame::decode(
        &std::fs::read(native.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let mut runtime = SimulationRuntime::restore_vanilla(input)?;
    let before = runtime.saved_json()?;
    let original: serde_json::Value =
        serde_json::from_reader(File::open(native.join("final.world.json"))?)?;
    assert_ne!(
        before.pointer("/chunks/ITBL"),
        original.pointer("/chunks/ITBL")
    );
    let towns = runtime.town_house_caches().clone();
    let roads = runtime.road_caches().clone();
    let content = runtime.content().clone();
    let derived = runtime.world().derived().clone();
    assert!(matches!(runtime.advance_world(1),
        Err(WorldTickError::Unsupported { phase: "industry_day", reason })
            if reason.contains("automatic industry creation settings")));
    assert_eq!(runtime.saved_json()?, before);
    assert_eq!(runtime.world().derived(), &derived);
    assert_eq!(runtime.content(), &content);
    assert_eq!(runtime.road_caches(), &roads);
    assert_eq!(runtime.town_house_caches(), &towns);
    Ok(())
}

#[test]
#[ignore = "requires actual 60680-call original capture; historical refusal selector retained without rollback credit"]
fn natural_late_disaster_rolls_back_complete_forest_runtime() -> Result {
    let input = World::decode(&Savegame::decode(
        &std::fs::read(std::env::var("OTTD_INDUSTRY_ROLLBACK_SAVE")?)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let capture = DisasterCapture::required(&input)?;
    let mut runtime = SimulationRuntime::restore_vanilla(input.clone())?;
    capture.compare(&runtime, "initial")?;
    let before = runtime.saved_json()?;
    runtime.advance_world(8192)?;
    assert_ne!(
        runtime.saved_json()?.pointer("/chunks/INDY"),
        before.pointer("/chunks/INDY")
    );
    capture.compare(&runtime, "prefix")?;
    let mut runtime = SimulationRuntime::restore_vanilla(input)?;
    runtime.advance_world(60_680)?;
    capture.finish(&runtime, 8192)
}
