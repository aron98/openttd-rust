//! Source-bound original company naming prerequisite for populated-world consumers.
use ottd_save::{Savegame, world::World};
use ottd_sim::{WorldTickError, advance_world};
use std::path::{Path, PathBuf};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn load(path: &Path) -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        &std::fs::read(path)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}

fn company_number(world: &serde_json::Value, name: &str) -> Result<u64> {
    world
        .pointer(&format!("/chunks/PLYR/records/0/{name}"))
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "missing native company number".into())
}

#[test]
#[ignore = "requires original-command populated house preparation and depot control"]
fn original_inert_name_passes_and_active_generation_remains_explicit() -> Result {
    let directory = PathBuf::from(std::env::var("OTTD_HOUSE_PROOF_DIR")?);
    let mut inert = load(&directory.join("native-red-256/initial.sav"))?;
    let before = inert.saved_json()?;
    assert_eq!(company_number(&before, "name_1")?, 24577);
    assert_eq!(company_number(&before, "last_build_coordinate")?, 0);
    advance_world(&mut inert, 1)?;
    assert_eq!(company_number(&inert.saved_json()?, "name_1")?, 24577);
    assert_eq!(
        company_number(&inert.saved_json()?, "last_build_coordinate")?,
        0
    );
    let mut active = load(&directory.join("native-active-name/active-name.sav"))?;
    let before = active.saved_json()?;
    assert_eq!(company_number(&before, "name_1")?, 24577);
    assert_eq!(company_number(&before, "last_build_coordinate")?, 1000);
    assert!(matches!(
        advance_world(&mut active, 1),
        Err(WorldTickError::Unsupported { phase: "company", reason })
            if reason == "automatic name generation"
    ));
    assert_eq!(active.saved_json()?, before);
    let original = load(&directory.join("native-active-name/final.sav"))?.saved_json()?;
    assert_eq!(company_number(&original, "name_1")?, 28902);
    Ok(())
}

#[test]
#[ignore = "requires authentic industry preparation with original generated company name"]
fn generated_name_matches_original_and_unnamed_active_state_rolls_back() -> Result {
    use ottd_sim::runtime::SimulationRuntime;
    let directory = PathBuf::from(std::env::var("OTTD_INDUSTRY_DESIGN_DIR")?);
    let input = load(&directory.join("generated-name-one/initial.sav"))?;
    assert_eq!(company_number(&input.saved_json()?, "name_1")?, 28902);
    assert_eq!(
        company_number(&input.saved_json()?, "last_build_coordinate")?,
        331
    );
    let mut runtime = SimulationRuntime::restore_vanilla(input)?;
    runtime.advance_world(1)?;
    let original = load(&directory.join("generated-name-one/final.sav"))?;
    assert_eq!(runtime.saved_json()?, original.saved_json()?);
    assert_eq!(runtime.world().derived(), original.derived());
    let restored = SimulationRuntime::restore_vanilla(original)?;
    assert_eq!(runtime.town_house_caches(), restored.town_house_caches());
    assert_eq!(runtime.road_caches(), restored.road_caches());
    let input = load(&directory.join("manager-search/random-plant-7/final.sav"))?;
    let mut active = SimulationRuntime::restore_vanilla(input)?;
    let before = active.saved_json()?;
    assert_eq!(company_number(&before, "name_1")?, 0x6001);
    assert_eq!(company_number(&before, "last_build_coordinate")?, 331);
    let towns = active.town_house_caches().clone();
    let roads = active.road_caches().clone();
    let derived = active.world().derived().clone();
    assert!(matches!(active.advance_world(1),
        Err(WorldTickError::Unsupported { phase: "company", reason })
        if reason == "automatic name generation"));
    assert_eq!(active.saved_json()?, before);
    assert_eq!(active.world().derived(), &derived);
    assert_eq!(active.town_house_caches(), &towns);
    assert_eq!(active.road_caches(), &roads);
    Ok(())
}
