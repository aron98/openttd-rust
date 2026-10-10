use super::{Args, state};
use crate::{
    commands::{CargoCapacities, CommandCost, CommandError, CommandReturn},
    content::{EngineSpec, Price, RoadSpec, VehicleSpec},
    runtime::RoadVehicleContext,
    world_access::unsigned,
};
use ottd_save::world::World;

pub(super) struct Purchase {
    pub(super) state: crate::runtime::RoadBuildState,
    pub(super) cost: CommandCost,
    pub(super) capacities: CommandReturn,
}
fn exceeds_company_limit(world: &World, company: u8, unit: u16) -> Result<bool, CommandError> {
    Ok(crate::runtime::road_company_count(world, company)?
        >= unsigned(world, b"PATS", 0, "vehicle.max_roadveh")?
        || unit == 65535)
}
pub(super) fn validate(
    world: &World,
    company: u8,
    args: Args,
    context: &RoadVehicleContext<'_>,
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
    context.admit_restore(world, args.tile, args.client_id)?;
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
