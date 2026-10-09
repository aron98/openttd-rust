use super::{Args, CommandCost, CommandError, Outcome, Phase, Plan, land};
use crate::{
    commands::{
        CommandReceipt, CommandReturnPhases, pipeline,
        terrain_read::{MapSize, TerrainRead},
    },
    content::ContentCatalog,
};
use ottd_save::world::World;

pub(in crate::commands) fn estimate(
    world: &World,
    company: u8,
    args: Args,
) -> Result<Plan, CommandError> {
    let catalog = ContentCatalog::from_world(world)?;
    let outcome = land(
        &mut Phase::Estimate(TerrainRead::Committed(world)),
        company,
        args,
        catalog.prices(),
    )?;
    let returns = Some(outcome.result());
    Ok(Plan {
        cost: outcome.cost,
        edits: Vec::new(),
        returns,
    })
}
pub(in crate::commands) fn run(
    world: &mut World,
    company: u8,
    args: Args,
    estimate_only: bool,
) -> Result<CommandReceipt, CommandError> {
    let catalog = ContentCatalog::from_world(world)?;
    let test = land(
        &mut Phase::Estimate(TerrainRead::Committed(world)),
        company,
        args,
        catalog.prices(),
    )?;
    if estimate_only || !test.cost.success {
        let returns = test.result();
        return Ok(CommandReceipt {
            posted: test.cost.success,
            gate: None,
            test: Some(test.cost.clone()),
            exec: None,
            result: Some(test.cost),
            returns: Some(CommandReturnPhases {
                test: Some(returns),
                exec: None,
                result: Some(returns),
            }),
        });
    }
    let size = MapSize::from_world(world);
    let mut transaction = world.transaction();
    let exec = land(
        &mut Phase::Execute {
            transaction: &mut transaction,
            size,
        },
        company,
        args,
        catalog.prices(),
    )?;
    let result = final_cost(&exec);
    if result.success {
        let view = TerrainRead::Candidate {
            view: transaction.view(),
            size,
        };
        for edit in pipeline::completion_edits(view, u32::from(company), args.tile, &result)? {
            transaction.apply(edit)?;
        }
    }
    let returns = CommandReturnPhases {
        test: Some(test.result()),
        exec: Some(exec.result()),
        result: Some(exec.result()),
    };
    transaction.prepare()?.commit();
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
