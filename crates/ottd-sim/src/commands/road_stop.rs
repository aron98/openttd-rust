mod acceptance;
mod args;
mod effects;
mod naming;
mod planning;
mod record;
use super::{CommandCost, CommandError, CommandReceipt, pipeline, terrain_read::TerrainRead};
use crate::{
    runtime::{StationContext, stations},
    world_access::{signed, unsigned},
};
pub use args::BuildRoadStop;
use ottd_save::world::World;

fn receipt(test: CommandCost, result: CommandCost, executed: bool) -> CommandReceipt {
    CommandReceipt {
        posted: result.success,
        gate: None,
        test: Some(test),
        exec: executed.then(|| result.clone()),
        result: Some(result),
        returns: None,
    }
}
pub(super) fn run(
    world: &mut World,
    company: u8,
    tile: u32,
    args: BuildRoadStop,
    estimate: bool,
    context: StationContext<'_>,
) -> Result<CommandReceipt, CommandError> {
    let mut plan = planning::validate(world, company, tile, args, &context)?;
    if !plan.cost.success || estimate {
        return Ok(receipt(plan.cost.clone(), plan.cost, false));
    }
    if plan.cost.cost > 0
        && unsigned(world, b"PATS", 0, "difficulty.infinite_money")? == 0
        && plan.cost.cost > signed(world, b"PLYR", u32::from(company), "money")?
    {
        let mut result = plan.cost.clone();
        result.success = false;
        result.error = Some("STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY".into());
        result.error_params = vec![result.cost];
        return Ok(receipt(plan.cost, result, false));
    }
    let mut state = context.stations.clone();
    let joining = args.station_to_join != 65533;
    let station = if joining {
        u32::from(args.station_to_join)
    } else {
        state
            .stations
            .allocate()
            .map_err(crate::runtime::RuntimeError::from)?
    };
    let stop = state
        .stops
        .allocate()
        .map_err(crate::runtime::RuntimeError::from)?;
    let geometry = state.geometry.entry(station).or_default();
    geometry.add(
        tile,
        args.stop_type == 0,
        (world.map().width(), world.map().height()),
        stations::catchment_radius(world)?,
    )?;
    plan.edits.extend(acceptance::zero_acceptance(
        world,
        geometry,
        joining.then_some(station),
    )?);
    let addition = effects::Addition {
        station,
        stop,
        tile,
        company,
        args,
    };
    if joining {
        addition.join(world, &mut plan.edits)?;
    } else {
        addition.new_station(world, &mut plan.edits)?;
    }
    addition.tile_and_stop(world, &mut plan.edits)?;
    let infrastructure = state
        .infrastructure
        .get_mut(&company)
        .ok_or(CommandError::Unsupported("station infrastructure company"))?;
    *infrastructure = infrastructure
        .checked_add(1)
        .ok_or(CommandError::Overflow("station infrastructure"))?;
    plan.edits.extend(pipeline::completion_edits(
        TerrainRead::Committed(world),
        u32::from(company),
        tile,
        &plan.cost,
    )?);
    context.publish(
        world,
        plan.edits,
        stations::StationPublication {
            next: state,
            company,
            station,
        },
    )?;
    Ok(receipt(plan.cost.clone(), plan.cost, true))
}
