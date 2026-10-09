mod state;
use super::{
    CargoCapacities, CommandCost, CommandError, CommandReceipt, CommandReturn, CommandReturnPhases,
    pipeline, terrain_read::TerrainRead,
};
use crate::{
    content::{EngineSpec, Price, RoadSpec, VehicleSpec},
    runtime::{PurchaseContext, VehicleId},
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
struct Purchase {
    state: crate::runtime::RoadBuildState,
    cost: CommandCost,
    capacities: CommandReturn,
}
fn exceeds_company_limit(world: &World, company: u8, unit: u16) -> Result<bool, CommandError> {
    Ok(crate::runtime::road_company_count(world, company)?
        >= unsigned(world, b"PATS", 0, "vehicle.max_roadveh")?
        || unit == 65535)
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
    context: PurchaseContext<'_>,
) -> Result<CommandReceipt, CommandError> {
    let plan = match validate(world, company, args, &context)? {
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
    if unsigned(world, b"PATS", 0, "difficulty.infinite_money")? == 0
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
fn validate(
    world: &World,
    company: u8,
    args: Args,
    context: &PurchaseContext<'_>,
) -> Result<Result<Purchase, CommandCost>, CommandError> {
    let tile = world
        .map()
        .tiles()
        .get(usize::try_from(args.tile).map_err(|_| CommandError::Overflow("depot tile"))?)
        .ok_or(CommandError::Unsupported("depot tile"))?;
    if tile.m1() & 31 != company {
        return Ok(Err(CommandCost::failure("CMD_ERROR")));
    }
    if (tile.tile_type() >> 4 == 1 && tile.m5() >> 6 == 3)
        || (tile.tile_type() >> 4 == 6 && tile.m5() >> 4 == 3)
        || (tile.tile_type() >> 4 == 5 && (tile.m6() >> 3) & 15 == 1)
    {
        return Err(CommandError::Unsupported(
            "non-road depot or airport construction",
        ));
    }
    if tile.tile_type() >> 4 != 2 || tile.m5() >> 6 != 2 {
        return Ok(Err(CommandCost::failure("CMD_ERROR")));
    }
    let Some(engine) = context.content.engines().get(usize::from(args.engine)) else {
        return Ok(Err(CommandCost::failure(
            "STR_ERROR_ROAD_VEHICLE_NOT_AVAILABLE",
        )));
    };
    let spec = match engine.vehicle {
        VehicleSpec::Road(spec) => spec,
        VehicleSpec::Rail(_) | VehicleSpec::Ship(_) | VehicleSpec::Aircraft(_) => {
            return Ok(Err(CommandCost::failure(
                "STR_ERROR_ROAD_VEHICLE_NOT_AVAILABLE",
            )));
        }
    };
    let available = unsigned(world, b"ENGN", u32::from(args.engine), "company_avail")?
        & (1_u64 << company)
        != 0;
    let climate = unsigned(world, b"PATS", 0, "game_creation.landscape")?;
    if !available
        || u64::from(engine.info.climates)
            & 1_u64
                .checked_shl(u32::try_from(climate).map_err(|_| CommandError::Overflow("climate"))?)
                .unwrap_or(0)
            == 0
    {
        return Ok(Err(CommandCost::failure(
            "STR_ERROR_ROAD_VEHICLE_NOT_AVAILABLE",
        )));
    }
    if args.cargo >= 64 && args.cargo != 255 || engine.info.cargo_type == 255 {
        return Ok(Err(CommandCost::failure("CMD_ERROR")));
    }
    if args.cargo != 255 && args.cargo != engine.info.cargo_type {
        return Err(CommandError::Unsupported("vehicle refit construction"));
    }
    if !context.allocation.pool.can_allocate(1) {
        return Ok(Err(CommandCost::failure(
            "STR_ERROR_TOO_MANY_VEHICLES_IN_GAME",
        )));
    }
    let unit = context
        .allocation
        .road_units
        .get(&company)
        .map_or(1, crate::runtime::pools::UnitNumberAllocator::next_id);
    if exceeds_company_limit(world, company, unit)? {
        return Ok(Err(CommandCost::failure(
            "STR_ERROR_TOO_MANY_VEHICLES_IN_GAME",
        )));
    }
    let cost = CommandCost::success(
        context
            .content
            .price(Price::BuildVehicleRoad)
            .saturating_mul(i64::from(spec.cost_factor))
            >> 8,
        1,
    );
    if tile.m4() & 63 != 0 {
        let mut error = cost;
        error.success = false;
        error.error = Some("STR_ERROR_DEPOT_WRONG_DEPOT_TYPE".into());
        return Ok(Err(error));
    }
    if world
        .tables()
        .get(b"BKOR")
        .is_none_or(|t| !t.records().is_empty())
    {
        return Err(CommandError::Unsupported("vehicle order-backup restore"));
    }
    let state = state::new(world, company, args, engine, spec, unit, cost.cost)?;
    Ok(Ok(Purchase {
        state,
        cost,
        capacities: capacities(engine, spec)?,
    }))
}
fn capacities(engine: &EngineSpec, spec: RoadSpec) -> Result<CommandReturn, CommandError> {
    let mut capacities = [0; 64];
    *capacities
        .get_mut(usize::from(engine.info.cargo_type))
        .ok_or(CommandError::Unsupported("default cargo"))? = u32::from(spec.capacity);
    Ok(CommandReturn::Vehicle {
        vehicle: 0xFFFFF,
        capacity: u32::from(spec.capacity),
        mail_capacity: 0,
        cargo_capacities: Box::new(CargoCapacities(capacities)),
    })
}
