use super::{Action, Native, Path, Protocol, Result, Value, equal, io, model::NativeAction};
use crate::{
    Command, CommandMode, CommandReceipt, CommandRequest,
    commands::{
        level_land, terraform,
        terrain_context::TerrainContext,
        terrain_run::{self, Args},
        terrain_state::TerrainState,
    },
    content::ContentCatalog,
    execute_command,
};
use ottd_save::world::World;
use serde_json::json;

pub(super) fn observe(
    world: &mut World,
    request: &CommandRequest,
) -> Result<(CommandReceipt, Value)> {
    let catalog = ContentCatalog::from_world(world)?;
    let mut context = TerrainContext::new(request.company);
    let state = TerrainState::new(world, catalog.prices());
    let estimate = matches!(request.mode, CommandMode::Estimate);
    let receipt = match request.command {
        Command::LandscapeClear { tile } => {
            terrain_run::run(state, &mut context, Args::Clear(tile), estimate)?
        }
        Command::TerraformLand {
            tile,
            slope,
            dir_up,
        } => terrain_run::run(
            state,
            &mut context,
            Args::Terraform(terraform::Args {
                tile,
                mask: slope,
                up: dir_up,
            }),
            estimate,
        )?,
        Command::LevelLand {
            tile,
            start_tile,
            diagonal,
            level_mode,
        } => level_land::run_context(
            state,
            &mut context,
            level_land::Args {
                tile,
                start: start_tile,
                diagonal,
                mode: level_mode,
            },
            estimate,
        )?,
        _ => return Err("non-terrain command in frozen terrain manifest".into()),
    };
    Ok((receipt, serde_json::to_value(context.events)?))
}

pub(super) fn run(run: &Path, compare_trace: bool, evidence: &str) -> Result<Value> {
    let protocol: Protocol = io::json(&run.join("actions.json"))?;
    let native: Native = io::json(&run.join("results.json"))?;
    assert_eq!(protocol.schema_version, 1);
    assert_eq!(native.schema_version, 1);
    assert_eq!(protocol.actions.len(), native.actions.len());
    let mut public = io::load(&run.join("initial.sav"))?;
    let mut observed = io::load(&run.join("initial.sav"))?;
    io::checkpoint(&public, run, "initial", evidence)?;
    io::checkpoint(&observed, run, "initial", evidence)?;
    let mut commands = 0_usize;
    let mut events = 0_usize;
    for (action, expected) in protocol.actions.iter().zip(&native.actions) {
        match (action, expected) {
            (
                Action::Command { ordinal, request },
                NativeAction::Command {
                    ordinal: expected_ordinal,
                    receipt,
                    native_metadata,
                },
            ) => {
                assert_eq!(ordinal, expected_ordinal);
                let public_receipt = execute_command(&mut public, request)?;
                let (observed_receipt, trace) = observe(&mut observed, request)?;
                let label = format!("{evidence}-{ordinal}");
                io::record(
                    &label,
                    &json!({
                        "receipt": observed_receipt, "public_receipt": public_receipt, "events": trace,
                    }),
                )?;
                equal(
                    &serde_json::to_value(public_receipt)?,
                    receipt,
                    &label,
                    "public receipt",
                )?;
                equal(
                    &serde_json::to_value(observed_receipt)?,
                    receipt,
                    &label,
                    "observed receipt",
                )?;
                equal(
                    &public.saved_json()?,
                    &observed.saved_json()?,
                    &label,
                    "dispatch saved world",
                )?;
                equal(
                    &serde_json::to_value(public.derived())?,
                    &serde_json::to_value(observed.derived())?,
                    &label,
                    "dispatch derived state",
                )?;
                if compare_trace {
                    let expected_trace = native_metadata
                        .tree_rating
                        .as_ref()
                        .ok_or("missing observed native trace")?;
                    equal(&trace, &expected_trace.events, &label, "trace")?;
                }
                commands = commands.saturating_add(1);
                events =
                    events.saturating_add(trace.as_array().ok_or("trace is not an array")?.len());
            }
            (
                Action::Checkpoint { ordinal, label },
                NativeAction::Checkpoint {
                    ordinal: expected_ordinal,
                },
            ) => {
                assert_eq!(ordinal, expected_ordinal);
                io::checkpoint(&public, run, label, evidence)?;
                io::checkpoint(&observed, run, label, evidence)?;
            }
            _ => return Err("protocol/native action kind mismatch".into()),
        }
    }
    io::checkpoint(&public, run, "final", evidence)?;
    io::checkpoint(&observed, run, "final", evidence)?;
    println!("NATIVE_CORPUS_CASE {evidence} commands={commands} events={events}");
    Ok(
        json!({"case": evidence, "commands": commands, "events": events,
              "receipt_saved_derived_match": true, "trace_compared": compare_trace}),
    )
}
