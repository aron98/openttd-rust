#[cfg(test)]
mod corpus;
#[cfg(test)]
mod tests;
#[cfg(test)]
use super::terrain_context::Phase;
use super::{
    Command, CommandError, CommandReceipt, CommandReturnPhases, Plan, pipeline, terraform,
    terrain_context::{ClearRequest, TerrainContext, TerrainFlags},
    terrain_state::TerrainState,
    tree_clear,
};
use crate::content::ContentCatalog;
use ottd_save::world::World;

#[derive(Clone, Copy)]
pub(super) enum Args {
    Clear(u32),
    Terraform(terraform::Args),
}
impl Args {
    pub(super) fn from_command(
        world: &World,
        command: &Command,
    ) -> Result<Option<Self>, CommandError> {
        match *command {
            Command::LandscapeClear { tile } => Ok((super::landscape::tile_at(world, tile)?
                .tile_type()
                >> 4
                == 4)
                .then_some(Self::Clear(tile))),
            Command::TerraformLand {
                tile,
                slope,
                dir_up,
            } => Ok(Some(Self::Terraform(terraform::Args {
                tile,
                mask: slope,
                up: dir_up,
            }))),
            _ => Ok(None),
        }
    }
    const fn tile(self) -> u32 {
        match self {
            Self::Clear(tile) => tile,
            Self::Terraform(args) => args.tile,
        }
    }
    fn body(
        self,
        state: &mut TerrainState<'_, '_>,
        context: &mut TerrainContext,
        execute: bool,
    ) -> Result<Plan, CommandError> {
        let flags = TerrainFlags(u16::from(execute));
        match self {
            Self::Clear(tile) => {
                tree_clear::body(state, context, ClearRequest { tile, flags }).map(Plan::empty)
            }
            Self::Terraform(args) => {
                terraform::body(state, context, args, TerrainFlags(flags.0 | 0x0102))
            }
        }
    }
}
pub(super) fn execute(
    world: &mut World,
    company: u8,
    args: Args,
    estimate: bool,
) -> Result<CommandReceipt, CommandError> {
    let catalog = ContentCatalog::from_world(world)?;
    let mut context = TerrainContext::new(company);
    run(
        TerrainState::new(world, catalog.prices()),
        &mut context,
        args,
        estimate,
    )
}
pub(super) fn run(
    mut state: TerrainState<'_, '_>,
    context: &mut TerrainContext,
    args: Args,
    estimate: bool,
) -> Result<CommandReceipt, CommandError> {
    let test = context.test(|context| {
        let outcome = args.body(&mut state, context, false)?;
        #[cfg(test)]
        context.phase(Phase::Test);
        Ok(outcome)
    })?;
    let mut result = test.cost.clone();
    let mut returns = test.returns.clone().map(|values| CommandReturnPhases {
        test: Some(values.clone()),
        exec: None,
        result: Some(values),
    });
    if result.success
        && !estimate
        && result.cost > 0
        && state
            .read()
            .unsigned(*b"PATS", 0, "difficulty.infinite_money")?
            == 0
        && result.cost
            > state
                .read()
                .signed(*b"PLYR", u32::from(context.company), "money")?
    {
        result.success = false;
        result.error = Some("STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY".into());
        result.error_params = vec![result.cost];
    }
    if !result.success || estimate {
        #[cfg(test)]
        context.phase(Phase::Result);
        return Ok(CommandReceipt {
            posted: result.success,
            gate: None,
            test: Some(test.cost),
            exec: None,
            result: Some(result),
            returns,
        });
    }
    let exec = args.body(&mut state, context, true)?;
    #[cfg(test)]
    context.phase(Phase::Exec);
    if exec.cost.success {
        let edits = pipeline::completion_edits(
            state.read(),
            u32::from(context.company),
            args.tile(),
            &exec.cost,
        )?;
        state.apply(edits)?;
        state.commit()?;
    }
    #[cfg(test)]
    context.phase(Phase::Result);
    if let Some(values) = &mut returns {
        values.exec.clone_from(&exec.returns);
        values.result = exec.returns;
    }
    Ok(CommandReceipt {
        posted: exec.cost.success,
        gate: None,
        test: Some(test.cost),
        exec: Some(exec.cost.clone()),
        result: Some(exec.cost),
        returns,
    })
}
