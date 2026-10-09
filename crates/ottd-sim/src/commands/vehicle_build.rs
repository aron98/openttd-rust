mod planning;
mod state;
use super::{
    CargoCapacities, CommandCost, CommandError, CommandReceipt, CommandReturn, CommandReturnPhases,
    pipeline, terrain_read::TerrainRead,
};
use crate::{
    runtime::{RoadVehicleContext, VehicleId},
    world_access::{field_edit, signed, unsigned},
};
use ottd_save::{
    WireValue,
    world::{World, WorldEdit},
};

#[derive(Clone, Copy)]
pub(super) struct Args {
    pub tile: u32,
    pub engine: u16,
    pub cargo: u8,
}
pub(super) fn empty_return() -> CommandReturn {
    CommandReturn::Vehicle {
        vehicle: 0xFFFFF,
        capacity: 0,
        mail_capacity: 0,
        cargo_capacities: Box::new(CargoCapacities([0; 64])),
    }
}
fn receipt(
    test: CommandCost,
    result: CommandCost,
    returns: CommandReturn,
    executed: bool,
) -> CommandReceipt {
    CommandReceipt {
        posted: result.success,
        gate: None,
        test: Some(test),
        exec: executed.then(|| result.clone()),
        result: Some(result),
        returns: Some(CommandReturnPhases {
            test: Some(returns.clone()),
            exec: executed.then(|| returns.clone()),
            result: Some(returns),
        }),
    }
}
pub(super) fn run(
    world: &mut World,
    company: u8,
    args: Args,
    estimate: bool,
    context: RoadVehicleContext<'_>,
) -> Result<CommandReceipt, CommandError> {
    let plan = match planning::validate(world, company, args, &context)? {
        Ok(plan) => plan,
        Err(cost) => return Ok(receipt(cost.clone(), cost, empty_return(), false)),
    };
    if estimate {
        return Ok(receipt(
            plan.cost.clone(),
            plan.cost,
            plan.capacities,
            false,
        ));
    }
    if plan.cost.cost > 0
        && unsigned(world, b"PATS", 0, "difficulty.infinite_money")? == 0
        && plan.cost.cost > signed(world, b"PLYR", u32::from(company), "money")?
    {
        let mut result = plan.cost.clone();
        result.success = false;
        result.error = Some("STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY".into());
        result.error_params = vec![result.cost];
        return Ok(receipt(plan.cost, result, plan.capacities, false));
    }
    let mut allocation = context.allocation.clone();
    let id = allocation
        .pool
        .allocate()
        .map_err(crate::runtime::RuntimeError::from)?;
    allocation
        .road_units
        .entry(company)
        .or_default()
        .use_id(plan.state.unit);
    let mut rng = ottd_core::Randomizer::from_state([
        u32::try_from(unsigned(world, b"DATE", 0, "random_state[0]")?)
            .map_err(|_| CommandError::Overflow("random seed"))?,
        u32::try_from(unsigned(world, b"DATE", 0, "random_state[1]")?)
            .map_err(|_| CommandError::Overflow("random seed"))?,
    ]);
    let random_bits = u16::try_from(rng.next_u32() & 65535)
        .map_err(|_| CommandError::Overflow("vehicle random bits"))?;
    let record = crate::runtime::new_road_record(
        world
            .tables()
            .get(b"VEHS")
            .ok_or(CommandError::Unsupported("vehicle schema"))?
            .schema(),
        crate::runtime::RoadBuildState {
            random_bits,
            ..plan.state
        },
        context.serializer_cargo_paid_for,
    )?;
    let mut edits = pipeline::completion_edits(
        TerrainRead::Committed(world),
        u32::from(company),
        args.tile,
        &plan.cost,
    )?;
    edits.push(WorldEdit::InsertRecord {
        chunk: *b"VEHS",
        record: id,
        value: record,
    });
    for (name, value) in ["random_state[0]", "random_state[1]"]
        .into_iter()
        .zip(rng.state())
    {
        edits.push(field_edit(
            *b"DATE",
            0,
            name,
            WireValue::Unsigned(u64::from(value)),
        ));
    }
    context.publish(world, edits, allocation, VehicleId::new(id))?;
    let mut result_returns = plan.capacities.clone();
    if let CommandReturn::Vehicle { vehicle, .. } = &mut result_returns {
        *vehicle = id;
    }
    let mut result = receipt(plan.cost.clone(), plan.cost, result_returns, true);
    if let Some(returns) = &mut result.returns {
        returns.test = Some(plan.capacities);
    }
    Ok(result)
}
