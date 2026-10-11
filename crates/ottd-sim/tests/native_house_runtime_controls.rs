//! Whole-transaction refusal after real house, cargo, year and script RNG work.
use ottd_save::{Savegame, world::World};
use ottd_sim::{
    WorldTickError,
    runtime::{RuntimeSaveContext, SimulationRuntime},
};
use std::path::PathBuf;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn world() -> Result<World> {
    let path = PathBuf::from(std::env::var("OTTD_HOUSE_ROLLBACK_SAVE")?);
    Ok(World::decode(&Savegame::decode(
        &std::fs::read(path)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}

#[test]
#[ignore = "requires original populated stage-zero house save"]
fn late_native_valid_industry_boundary_preserves_complete_house_runtime() -> Result {
    let mut prefix = SimulationRuntime::restore_vanilla(world()?)?;
    let initial = prefix.saved_json()?;
    let initial_cache = prefix.town_house_caches().clone();
    prefix.advance_world(6144)?;
    assert_ne!(prefix.town_house_caches(), &initial_cache);
    assert_ne!(
        prefix.saved_json()?.pointer("/chunks/CITY"),
        initial.pointer("/chunks/CITY")
    );
    assert_ne!(
        prefix.saved_json()?.pointer("/chunks/SRND/records/16"),
        initial.pointer("/chunks/SRND/records/16")
    );
    let mut runtime = SimulationRuntime::restore_vanilla(world()?)?;
    let derived = runtime.world().derived().clone();
    let content = runtime.content().clone();
    let roads = runtime.road_caches().clone();
    assert!(matches!(
        runtime.advance_world(36_778),
        Err(WorldTickError::Unsupported {
            phase: "industry_day",
            ..
        })
    ));
    assert_eq!(runtime.saved_json()?, initial);
    assert_eq!(runtime.world().derived(), &derived);
    assert_eq!(runtime.town_house_caches(), &initial_cache);
    assert_eq!(runtime.content(), &content);
    assert_eq!(runtime.road_caches(), &roads);
    Ok(())
}

#[test]
#[ignore = "requires original populated house save"]
fn network_ticks_remain_explicit_and_zero_calls_are_inert() -> Result {
    for context in [
        RuntimeSaveContext::NetworkClient,
        RuntimeSaveContext::NetworkServer,
    ] {
        let (mut runtime, _) = SimulationRuntime::from_loaded_vanilla(world()?, context)?;
        let before = runtime.saved_json()?;
        let caches = runtime.town_house_caches().clone();
        assert!(runtime.advance_world(0)?.ticks.is_empty());
        assert!(matches!(
            runtime.advance_world(1),
            Err(WorldTickError::Unsupported { phase: "host", .. })
        ));
        assert_eq!(runtime.saved_json()?, before);
        assert_eq!(runtime.town_house_caches(), &caches);
    }
    Ok(())
}

#[test]
#[ignore = "requires original animation and custom growth command controls"]
fn original_animation_and_growth_controls_are_transactionally_refused() -> Result {
    let proof = PathBuf::from(std::env::var("OTTD_HOUSE_CONTROL_DIR")?);
    for (case, phase) in [("animation-256", "domain"), ("growth-256", "town")] {
        let directory = proof.join(case);
        let input = World::decode(&Savegame::decode(
            &std::fs::read(directory.join("initial.sav"))?,
            ottd_save::DEFAULT_MAX_BYTES,
        )?)?;
        let mut runtime = SimulationRuntime::restore_vanilla(input)?;
        let before = runtime.saved_json()?;
        let native: serde_json::Value =
            serde_json::from_reader(std::fs::File::open(directory.join("final.world.json"))?)?;
        assert_ne!(before, native);
        if case == "animation-256" {
            assert_ne!(
                before.pointer("/chunks/ANIT"),
                native.pointer("/chunks/ANIT")
            );
        }
        let derived = runtime.world().derived().clone();
        let caches = runtime.town_house_caches().clone();
        assert!(matches!(runtime.advance_world(256),
            Err(WorldTickError::Unsupported { phase: actual, .. }) if actual == phase));
        assert_eq!(runtime.saved_json()?, before);
        assert_eq!(runtime.world().derived(), &derived);
        assert_eq!(runtime.town_house_caches(), &caches);
    }
    Ok(())
}
