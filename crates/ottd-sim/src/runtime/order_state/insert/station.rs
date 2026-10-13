use super::{
    CommandCost, OrderContext, RuntimeError,
    validation::{Insertion, owned},
};
use crate::runtime::{VehicleId, order_state::view};
use ottd_save::world::World;

pub(super) fn validate(
    world: &World,
    context: &OrderContext<'_>,
    args: &Insertion,
) -> Result<CommandCost, RuntimeError> {
    let station_id = u32::from(args.order.dest);
    let table = view::table(world, *b"STNN")?;
    let Some(record) = table.records().get(&station_id) else {
        return Ok(CommandCost::failure("CMD_ERROR"));
    };
    let station = view::Row {
        schema: table.schema(),
        record,
    };
    if station.number("facilities")? & 128 != 0 {
        return Ok(CommandCost::failure("CMD_ERROR"));
    }
    let (schema, records) = station.children("normal")?;
    let [record] = records else {
        return Err(RuntimeError::Invalid("station normal record"));
    };
    let (schema, records) = (view::Row { schema, record }).children("base")?;
    let [record] = records else {
        return Err(RuntimeError::Invalid("station base record"));
    };
    let owner = (view::Row { schema, record }).number("owner")?;
    if owner != 16 {
        let result = owned(owner, args.company);
        if !result.success {
            return Ok(result);
        }
    }
    let compatible = |id| usable(world, context, (station_id, id));
    if let Some(reason) = compatible(args.vehicle)? {
        return Ok(unusable("STR_ERROR_CAN_T_ADD_ORDER", reason));
    }
    let list = view::consist(world, args.vehicle.raw())?
        .number("orders")?
        .checked_sub(1);
    if let Some(list) = list {
        let list = world
            .derived()
            .order_lists
            .iter()
            .find(|l| u64::from(l.id) == list)
            .ok_or(RuntimeError::Invalid("station shared orders"))?;
        for id in &list.vehicles {
            if let Some(reason) = compatible(VehicleId::new(*id))? {
                return Ok(unusable("STR_ERROR_CAN_T_ADD_ORDER_SHARED", reason));
            }
        }
    }
    Ok(CommandCost::success(0, 255))
}
fn unusable(symbol: &str, reason: &str) -> CommandCost {
    let mut cost = CommandCost::failure(symbol);
    cost.extra_error = Some(reason.into());
    cost
}
fn usable(
    world: &World,
    context: &OrderContext<'_>,
    target: (u32, VehicleId),
) -> Result<Option<&'static str>, RuntimeError> {
    let (station, id) = target;
    let vehicle = view::consist(world, id.raw())?;
    if vehicle.number("next")? != 0 {
        return Err(RuntimeError::Unsupported("articulated station order"));
    }
    let cargo = usize::try_from(vehicle.number("cargo_type")?)
        .map_err(|_| RuntimeError::Invalid("order cargo"))?;
    let bus = context
        .content
        .cargo()
        .get(cargo)
        .ok_or(RuntimeError::Invalid("order cargo"))?
        .classes
        & 1
        != 0;
    let cache = context
        .road
        .get(&id)
        .ok_or(RuntimeError::Invalid("station road cache"))?;
    let kind = if bus { "bus" } else { "truck" };
    let mut reason = if bus {
        "STR_ERROR_NO_BUS_STATION"
    } else {
        "STR_ERROR_NO_TRUCK_STATION"
    };
    let chain = world
        .derived()
        .road_stop_chains
        .iter()
        .find(|chain| chain.station == station && chain.kind == kind)
        .ok_or(RuntimeError::Invalid("station roadstop chain"))?;
    for stop in &chain.stops {
        let stop = view::row(world, *b"ROAD", *stop)?;
        let tile = usize::try_from(stop.number("xy")?)
            .map_err(|_| RuntimeError::Invalid("order roadstop tile"))?;
        let tile = world
            .map()
            .tiles()
            .get(tile)
            .ok_or(RuntimeError::Invalid("order roadstop tile"))?;
        let road = tile.m4() & 63;
        let tram = (tile.m8() >> 6) & 63;
        if (road != 63 && cache.compatible_roadtypes & (1_u64 << road) != 0)
            || (tram != 63 && cache.compatible_roadtypes & (1_u64 << tram) != 0)
        {
            return Ok(None);
        }
        reason = if cache.road_type == 1 {
            "STR_ERROR_NO_STOP_COMPATIBLE_TRAM_TYPE"
        } else {
            "STR_ERROR_NO_STOP_COMPATIBLE_ROAD_TYPE"
        };
    }
    Ok(Some(reason))
}
