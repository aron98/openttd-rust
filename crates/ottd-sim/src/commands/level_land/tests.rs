use super::{Args, TerrainContext, TerrainState, phases};
use crate::{Command, CommandMode, CommandRequest, content::ContentCatalog, execute_command};
use ottd_save::{Savegame, world::World};
use std::path::Path;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn load(path: &Path) -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        &std::fs::read(path)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}

#[test]
#[ignore = "requires the retained native24 tree audit directory"]
fn native_level_cases_match_world_receipt_and_scopes() -> Result {
    let directory = std::env::var("TREE_NATIVE_AUDIT")?;
    for case in ["level-one", "level-money"] {
        let run = Path::new(&directory).join("runs").join(case);
        let actions: serde_json::Value =
            serde_json::from_slice(&std::fs::read(run.join("actions.json"))?)?;
        let request: CommandRequest = serde_json::from_value(
            actions
                .pointer("/actions/0/request")
                .ok_or("request")?
                .clone(),
        )?;
        let native: serde_json::Value =
            serde_json::from_slice(&std::fs::read(run.join("results.json"))?)?;
        let expected_world: serde_json::Value =
            serde_json::from_slice(&std::fs::read(run.join("final.world.json"))?)?;
        let expected_derived: serde_json::Value =
            serde_json::from_slice(&std::fs::read(run.join("final.derived.json"))?)?;
        let mut world = load(&run.join("initial.sav"))?;
        let receipt = execute_command(&mut world, &request)?;
        assert_eq!(
            serde_json::to_value(&receipt)?,
            native
                .pointer("/actions/0/receipt")
                .ok_or("receipt")?
                .clone(),
            "{case} receipt"
        );
        assert_eq!(world.saved_json()?, expected_world, "{case} full world");
        assert_eq!(
            serde_json::to_value(world.derived())?,
            expected_derived,
            "{case} derived"
        );
        let Command::LevelLand {
            tile,
            start_tile,
            diagonal,
            level_mode,
        } = request.command
        else {
            return Err("expected LevelLand request".into());
        };
        let mut observed_world = load(&run.join("initial.sav"))?;
        let catalog = ContentCatalog::from_world(&observed_world)?;
        let mut context = TerrainContext::new(request.company);
        let observed = phases::run_context(
            TerrainState::new(&mut observed_world, catalog.prices()),
            &mut context,
            Args {
                tile,
                start: start_tile,
                diagonal,
                mode: level_mode,
            },
            matches!(request.mode, CommandMode::Estimate),
        )?;
        if let Some(directory) = std::env::var_os("TREE_RUST_TRACE_DIR") {
            std::fs::create_dir_all(&directory)?;
            std::fs::write(
                Path::new(&directory).join(format!("{case}.json")),
                serde_json::to_vec_pretty(
                    &serde_json::json!({"receipt": observed, "events": context.events}),
                )?,
            )?;
        }
        assert_eq!(observed, receipt, "{case} observed receipt");
        assert_eq!(
            observed_world.saved_json()?,
            expected_world,
            "{case} observed full world"
        );
        assert_eq!(
            serde_json::to_value(&context.events)?,
            native
                .pointer("/actions/0/native_metadata/tree_rating/events")
                .ok_or("events")?
                .clone(),
            "{case} actual trace"
        );
        println!("NATIVE_LEVEL_MATCH {case} events={}", context.events.len());
    }
    Ok(())
}

#[test]
#[ignore = "requires the retained native24 tree audit directory"]
fn execution_scope_error_discards_already_staged_city_and_terrain() -> Result {
    let directory = std::env::var("TREE_NATIVE_AUDIT")?;
    let mut world = load(&Path::new(&directory).join("runs/level-one/initial.sav"))?;
    let mut high = ottd_save::TileRawParts::from(world.map().tiles().get(652).ok_or("tile")?);
    high.height = 5;
    world.edit_tile(652, &high.into())?;
    let mut water = ottd_save::TileRawParts::from(world.map().tiles().get(648).ok_or("tile")?);
    water.tile_type = 0x60;
    water.m5 = 0;
    world.edit_tile(648, &water.into())?;
    let before = world.saved_json()?;
    let catalog = ContentCatalog::from_world(&world)?;
    let mut context = TerrainContext::new(0);
    let result = phases::run_context(
        TerrainState::new(&mut world, catalog.prices()),
        &mut context,
        Args {
            tile: 650,
            start: 652,
            diagonal: false,
            mode: 2,
        },
        false,
    );
    assert!(matches!(
        result,
        Err(crate::CommandError::Unsupported(
            "clearing non-clear terrain"
        ))
    ));
    assert!(!context.testing());
    assert!(context.events.iter().any(|entry| matches!(
        &entry.event,
        crate::commands::terrain_context::trace::Event::Applied {
            test_mode: false,
            saved_rating: 465,
            ..
        }
    )));
    assert_eq!(world.saved_json()?, before);
    if let Some(directory) = std::env::var_os("TREE_RUST_TRACE_DIR") {
        std::fs::create_dir_all(&directory)?;
        std::fs::write(
            Path::new(&directory).join("scope-rollback.json"),
            serde_json::to_vec_pretty(&context.events)?,
        )?;
    }
    println!(
        "LEVEL_SCOPE_ROLLBACK staged_rating=465 saved_world_unchanged=true events={}",
        context.events.len()
    );
    Ok(())
}
