use super::{Action, Protocol, Result, io};
use crate::commands::{
    level_land, terraform,
    terrain_context::TerrainContext,
    terrain_run::{self, Args},
    terrain_state::TerrainState,
};
use crate::{Command, CommandError, CommandMode, content::ContentCatalog, execute_command};
use ottd_save::{WireValue, world::PathElement};
use serde_json::json;
use std::path::PathBuf;

#[test]
#[ignore = "requires freshly reproduced original terrain cases; no raw-load native parity claim"]
fn raw_authority_coupled_preflight_refusal_cleans_scope() -> Result {
    let prior = PathBuf::from(std::env::var("TREE_NATIVE_AUDIT")?);
    for name in ["terraform-one", "level-one"] {
        let run = prior.join("runs").join(name);
        let protocol: Protocol = io::json(&run.join("actions.json"))?;
        let Some(Action::Command { request, .. }) = protocol.actions.first() else {
            return Err("expected one command".into());
        };
        for distance in [0_u8, 2] {
            for mode in [CommandMode::Estimate, CommandMode::Post] {
                let mut request = request.clone();
                request.mode = mode;
                let mut public = io::load(&run.join("initial.sav"))?;
                let mut observed = io::load(&run.join("initial.sav"))?;
                for world in [&mut public, &mut observed] {
                    world.edit_field(
                        *b"PATS",
                        0,
                        &[PathElement::Field("economy.dist_local_authority".into())],
                        WireValue::Unsigned(u64::from(distance)),
                    )?;
                }
                let before = public.saved_json()?;
                let derived = serde_json::to_value(public.derived())?;
                let actual = execute_command(&mut public, &request);
                let catalog = ContentCatalog::from_world(&observed)?;
                let mut context = TerrainContext::new(request.company);
                let state = TerrainState::new(&mut observed, catalog.prices());
                let estimate = matches!(mode, CommandMode::Estimate);
                let captured = match request.command {
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
                    ),
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
                    ),
                    _ => return Err("unexpected coupled command".into()),
                };
                let mode_name = match mode {
                    CommandMode::Estimate => "estimate",
                    CommandMode::Post => "post",
                };
                io::record(
                    &format!("coupled-{name}-{distance}-{mode_name}"),
                    &json!({"case":name,"raw_distance":distance,"mode":mode_name,"public":format!("{actual:?}"),"observed":format!("{captured:?}"),"events":context.events,"testing":context.testing(),"map_entries":context.ratings.len(),"native_raw_trace_parity":false}),
                )?;
                assert!(matches!(
                    actual,
                    Err(CommandError::Unsupported(
                        "noncanonical town authority distance"
                    ))
                ));
                assert!(matches!(
                    captured,
                    Err(CommandError::Unsupported(
                        "noncanonical town authority distance"
                    ))
                ));
                assert!(!context.testing());
                assert!(context.ratings.is_empty());
                for world in [&public, &observed] {
                    assert_eq!(world.saved_json()?, before);
                    assert_eq!(serde_json::to_value(world.derived())?, derived);
                }
                assert!(!context.events.is_empty());
            }
        }
    }
    Ok(())
}
