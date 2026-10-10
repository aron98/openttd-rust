use super::{CommandCost, CommandError, CommandReceipt, pipeline, terrain_read::TerrainRead};
use crate::{
    runtime::{RoadVehicleContext, VehicleId},
    world_access::{signed, unsigned},
};
use ottd_save::world::{World, WorldEdit};

#[derive(Clone, Copy)]
pub(super) struct Args {
    pub location: u32,
    pub vehicle: u32,
    pub backup_order: bool,
    pub client_id: u32,
}
fn receipt(test: CommandCost, result: CommandCost, executed: bool) -> CommandReceipt {
    CommandReceipt {
        posted: result.success,
        gate: None,
        exec: executed.then(|| result.clone()),
        test: Some(test),
        result: Some(result),
        returns: None,
    }
}
pub(super) fn run(
    world: &mut World,
    company: u8,
    args: Args,
    estimate: bool,
    context: RoadVehicleContext<'_>,
) -> Result<CommandReceipt, CommandError> {
    let test = context.sale_cost(
        world,
        company,
        VehicleId::new(args.vehicle),
        args.backup_order,
        args.client_id,
    )?;
    if !test.success || estimate {
        return Ok(receipt(test.clone(), test, false));
    }
    if test.cost > 0
        && unsigned(world, b"PATS", 0, "difficulty.infinite_money")? == 0
        && test.cost > signed(world, b"PLYR", u32::from(company), "money")?
    {
        let mut result = test.clone();
        result.success = false;
        result.error = Some("STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY".into());
        result.error_params = vec![result.cost];
        return Ok(receipt(test, result, false));
    }
    let mut edits = pipeline::completion_edits(
        TerrainRead::Committed(world),
        u32::from(company),
        args.location,
        &test,
    )?;
    edits.push(WorldEdit::RemoveRecord {
        chunk: *b"VEHS",
        record: args.vehicle,
    });
    context.publish_sale(
        world,
        edits,
        VehicleId::new(args.vehicle),
        args.backup_order.then_some(args.client_id),
    )?;
    Ok(receipt(test.clone(), test, true))
}
