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
    assert!(matches!(
        advance_world(&mut inert, 1),
        Err(WorldTickError::Unsupported {
            phase: "road_tile_loop",
            ..
        })
    ));
    assert_eq!(inert.saved_json()?, before);
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
