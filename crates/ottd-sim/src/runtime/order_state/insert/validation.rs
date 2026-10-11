use super::{CommandCost, OrderContext, RuntimeError};
use crate::{
    commands::InsertOrder,
    runtime::{VehicleId, order_state::view},
};
use ottd_save::world::World;

pub(super) struct Insertion {
    pub company: u8,
    pub location: u32,
    pub vehicle: VehicleId,
    pub index: u8,
    pub order: InsertOrder,
}
pub(super) fn owned(owner: u64, company: u8) -> CommandCost {
    if owner == u64::from(company) {
        return CommandCost::success(0, 255);
    }
    let mut cost = CommandCost::failure("STR_ERROR_OWNED_BY");
    cost.error_params = vec![0x881D, i64::try_from(owner).unwrap_or(i64::MAX)];
    cost
}
pub(super) fn validate(
    world: &World,
    context: &OrderContext<'_>,
    args: &Insertion,
) -> Result<CommandCost, RuntimeError> {
    let failure = || CommandCost::failure("CMD_ERROR");
    if !view::table(world, *b"VEHS")?
        .records()
        .contains_key(&args.vehicle.raw())
    {
        return Ok(failure());
    }
    let Some(("roadveh", vehicle)) =
        view::vehicle(view::row(world, *b"VEHS", args.vehicle.raw())?)?
    else {
        return Err(RuntimeError::Unsupported("non-road insertion"));
    };
    if vehicle.number("subtype")? & 1 == 0 {
        return Ok(failure());
    }
    let owner = owned(vehicle.number("owner")?, args.company);
    if !owner.success {
        return Ok(owner);
    }
    let order = args.order;
    if order.refit_cargo != 254
        || order.wait_time != 0
        || order.travel_time != 0
        || order.max_speed != u16::MAX
    {
        return Ok(failure());
    }
    if vehicle.number("next")? != 0 {
        return Err(RuntimeError::Unsupported("articulated order insertion"));
    }
    let orders = super::publication::existing(world, vehicle)?;
    match order.kind & 15 {
        2 => {
            let result = depot(world, args, &orders)?;
            if !result.success {
                return Ok(result);
            }
        }
        1 => {
            let result = super::station::validate(world, context, args)?;
            if !result.success {
                return Ok(result);
            }
            let load = (order.flags >> 4) & 7;
            if !matches!(load, 0 | 2 | 3 | 4) {
                return Ok(failure());
            }
            if matches!(load, 2 | 3) && has(&orders, 1)? {
                return Ok(CommandCost::failure("STR_ERROR_UNBUNCHING_NO_FULL_LOAD"));
            }
            if !matches!(order.flags & 7, 0 | 1 | 2 | 4) || (order.kind >> 4) & 3 != 2 {
                return Ok(failure());
            }
        }
        6 | 7 => {
            return Err(RuntimeError::Unsupported(
                "waypoint or conditional insertion",
            ));
        }
        _ => return Ok(failure()),
    }
    if usize::from(args.index) > orders.len() {
        return Ok(failure());
    }
    if orders.len() >= 254 {
        return Ok(CommandCost::failure("STR_ERROR_TOO_MANY_ORDERS"));
    }
    if vehicle.number("orders")? == 0 && !context.state.lists.can_allocate(1) {
        return Ok(CommandCost::failure("STR_ERROR_NO_MORE_SPACE_FOR_ORDERS"));
    }
    Ok(CommandCost::success(0, 255))
}
fn has(orders: &[view::Row<'_>], flag: u8) -> Result<bool, RuntimeError> {
    for row in orders {
        let kind = row.number("type")? & 15;
        let flags = row.number("flags")?;
        if match flag {
            0 => kind == 1 && matches!((flags >> 4) & 7, 2 | 3),
            1 => kind == 2 && flags & 32 != 0,
            2 => kind == 7,
            _ => false,
        } {
            return Ok(true);
        }
    }
    Ok(false)
}

fn depot(
    world: &World,
    args: &Insertion,
    orders: &[view::Row<'_>],
) -> Result<CommandCost, RuntimeError> {
    let failure = || CommandCost::failure("CMD_ERROR");
    let order = args.order;
    if order.flags & 16 == 0 {
        let Some(record) = view::table(world, *b"DEPT")?
            .records()
            .get(&u32::from(order.dest))
        else {
            return Ok(failure());
        };
        let table = view::table(world, *b"DEPT")?;
        let tile = (view::Row {
            schema: table.schema(),
            record,
        })
        .number("xy")?;
        let tile = world
            .map()
            .tiles()
            .get(usize::try_from(tile).map_err(|_| RuntimeError::Invalid("order depot tile"))?)
            .ok_or(RuntimeError::Invalid("order depot tile"))?;
        let owner = owned(u64::from(tile.m1() & 31), args.company);
        if !owner.success {
            return Ok(owner);
        }
        if tile.tile_type() >> 4 != 2 || tile.m5() >> 6 != 2 {
            return Ok(failure());
        }
    }
    let reason = order.flags & 7;
    let action = (order.flags >> 3) & 15;
    if (reason != 0 && reason != 2 && reason != 3)
        || action & !7 != 0
        || reason & 1 != 0 && action & 5 != 0
    {
        return Ok(failure());
    }
    if action & 4 != 0 {
        for (flag, error) in [
            (0, "STR_ERROR_UNBUNCHING_NO_UNBUNCHING_FULL_LOAD"),
            (1, "STR_ERROR_UNBUNCHING_ONLY_ONE_ALLOWED"),
            (2, "STR_ERROR_UNBUNCHING_NO_UNBUNCHING_CONDITIONAL"),
        ] {
            if has(orders, flag)? {
                return Ok(CommandCost::failure(error));
            }
        }
    }
    Ok(CommandCost::success(0, 255))
}
