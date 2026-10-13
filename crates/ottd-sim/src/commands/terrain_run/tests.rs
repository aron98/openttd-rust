use super::*;
use crate::{Command, CommandMode, CommandRequest, execute_command};
use ottd_save::Savegame;
use std::path::Path;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn record(case: &str, context: &TerrainContext, receipt: &CommandReceipt) -> Result {
    if let Some(directory) = std::env::var_os("TREE_RUST_TRACE_DIR") {
        std::fs::create_dir_all(&directory)?;
        std::fs::write(
            Path::new(&directory).join(format!("{case}.json")),
            serde_json::to_vec_pretty(
                &serde_json::json!({"receipt": receipt, "events": context.events}),
            )?,
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "requires the retained native24 tree audit directory"]
fn native_terraform_cases_match_public_world_and_receipt() -> Result {
    let directory = std::env::var("TREE_NATIVE_AUDIT")?;
    for case in [
        "terraform-one",
        "terraform-four",
        "terraform-estimate",
        "terraform-limit",
        "terraform-money",
        "shore-terraform",
    ] {
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
        let Command::TerraformLand {
            tile,
            slope,
            dir_up,
        } = request.command
        else {
            return Err("expected TerraformLand request".into());
        };
        let mut observed_world = load(&run.join("initial.sav"))?;
        let catalog = ContentCatalog::from_world(&observed_world)?;
        let mut context = TerrainContext::new(request.company);
        let observed = super::run(
            TerrainState::new(&mut observed_world, catalog.prices()),
            &mut context,
            Args::Terraform(terraform::Args {
                tile,
                mask: slope,
                up: dir_up,
            }),
            matches!(request.mode, CommandMode::Estimate),
        )?;
        record(case, &context, &observed)?;
        assert_eq!(observed, receipt, "{case} observed receipt");
        assert_eq!(
            observed_world.saved_json()?,
            expected_world,
            "{case} observed world"
        );
        assert_eq!(
            serde_json::to_value(&context.events)?,
            native
                .pointer("/actions/0/native_metadata/tree_rating/events")
                .ok_or("events")?
                .clone(),
            "{case} actual trace"
        );
        println!(
            "NATIVE_TERRAFORM_MATCH {case} events={}",
            context.events.len()
        );
    }
    Ok(())
}

fn load(path: &Path) -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        &std::fs::read(path)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}

#[test]
#[ignore = "requires the retained native24 tree audit directory"]
fn native_clear_cases_match_world_receipt_and_actual_rating_events() -> Result {
    let directory = std::env::var("TREE_NATIVE_AUDIT")?;
    for case in [
        "clear-on",
        "threshold-inside",
        "threshold-boundary",
        "threshold-outside",
        "clamp",
        "below",
        "magic",
        "shore-clear",
        "rainforest",
        "tie",
        "clear-limit",
    ] {
        let run = Path::new(&directory).join("runs").join(case);
        let native: serde_json::Value =
            serde_json::from_slice(&std::fs::read(run.join("results.json"))?)?;
        let expected_world: serde_json::Value =
            serde_json::from_slice(&std::fs::read(run.join("final.world.json"))?)?;
        let expected_derived: serde_json::Value =
            serde_json::from_slice(&std::fs::read(run.join("final.derived.json"))?)?;
        let mut world = load(&run.join("initial.sav"))?;
        let catalog = ContentCatalog::from_world(&world)?;
        let mut context = TerrainContext::new(0);
        let receipt = super::run(
            TerrainState::new(&mut world, catalog.prices()),
            &mut context,
            Args::Clear(650),
            false,
        )?;
        record(case, &context, &receipt)?;
        assert_eq!(
            serde_json::to_value(&receipt)?,
            native
                .pointer("/actions/0/receipt")
                .ok_or("receipt")?
                .clone(),
            "{case} receipt"
        );
        assert_eq!(
            serde_json::to_value(&context.events)?,
            native
                .pointer("/actions/0/native_metadata/tree_rating/events")
                .ok_or("events")?
                .clone(),
            "{case} trace"
        );
        assert_eq!(world.saved_json()?, expected_world, "{case} full world");
        assert_eq!(
            serde_json::to_value(world.derived())?,
            expected_derived,
            "{case} derived"
        );
        let mut public_world = load(&run.join("initial.sav"))?;
        let public = execute_command(
            &mut public_world,
            &CommandRequest {
                company: 0,
                mode: CommandMode::Post,
                command: Command::LandscapeClear { tile: 650 },
            },
        )?;
        assert_eq!(public, receipt, "{case} public dispatch");
        assert_eq!(
            public_world.saved_json()?,
            expected_world,
            "{case} public world"
        );
        println!("NATIVE_CLEAR_MATCH {case} events={}", context.events.len());
    }
    Ok(())
}
