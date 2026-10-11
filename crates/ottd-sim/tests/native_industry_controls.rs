//! Original-valid automatic industry policy remains an explicit transactional boundary.
use ottd_save::{Savegame, world::World};
use ottd_sim::{WorldTickError, runtime::SimulationRuntime};
use std::{fs::File, path::PathBuf};
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
