use super::{
    Args, CommandCost, CommandError, Outcome, TerrainContext, TerrainFlags, TerrainState, land,
};
#[cfg(test)]
use crate::commands::terrain_context::Phase;
use crate::{
    commands::{CommandReceipt, CommandReturnPhases, pipeline},
    content::ContentCatalog,
};
use ottd_save::world::World;

pub(in crate::commands) fn run(
    world: &mut World,
    company: u8,
    args: Args,
    estimate_only: bool,
) -> Result<CommandReceipt, CommandError> {
    let catalog = ContentCatalog::from_world(world)?;
    let mut context = TerrainContext::new(company);
    run_context(
        TerrainState::new(world, catalog.prices()),
        &mut context,
        args,
        estimate_only,
    )
}
pub(in crate::commands) fn run_context(
    mut state: TerrainState<'_, '_>,
    context: &mut TerrainContext,
    args: Args,
    estimate_only: bool,
) -> Result<CommandReceipt, CommandError> {
    let test = context.test(|context| {
        let result = land(&mut state, context, args, TerrainFlags(0x0102))?;
        #[cfg(test)]
        context.phase(Phase::Test);
        Ok(result)
    })?;
    if estimate_only || !test.cost.success {
        #[cfg(test)]
        context.phase(Phase::Result);
        let returns = test.result();
        return Ok(CommandReceipt {
            posted: test.cost.success,
            gate: None,
            test: Some(test.cost.clone()),
            exec: None,
            result: Some(test.cost),
            returns: Some(CommandReturnPhases {
                test: Some(returns.clone()),
                exec: None,
                result: Some(returns),
            }),
        });
    }
    let exec = land(&mut state, context, args, TerrainFlags(0x0103))?;
    #[cfg(test)]
    context.phase(Phase::Exec);
    let result = final_cost(&exec);
    if result.success {
        let edits = pipeline::completion_edits(
            state.read(),
            u32::from(context.company),
            args.tile,
            &result,
        )?;
        state.apply(edits)?;
    }
    let returns = CommandReturnPhases {
        test: Some(test.result()),
        exec: Some(exec.result()),
        result: Some(exec.result()),
    };
    state.commit()?;
    #[cfg(test)]
    context.phase(Phase::Result);
    Ok(CommandReceipt {
        posted: result.success,
        gate: None,
        test: Some(test.cost),
        exec: Some(exec.cost),
        result: Some(result),
        returns: Some(returns),
    })
}
fn final_cost(exec: &Outcome) -> CommandCost {
    if exec.cost.success && exec.additional_money != 0 && exec.cost.cost == 0 {
        CommandCost::parameterized(
            "STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY",
            exec.additional_money,
        )
    } else {
        exec.cost.clone()
    }
}
