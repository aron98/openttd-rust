use super::{RoadVehicleContext, RuntimeError, SavedVehicleView, VehicleId};
use crate::{CommandCost, CommandError};
use ottd_save::{
    WireValue,
    world::{World, WorldEdit},
};

impl RoadVehicleContext<'_> {
    pub(crate) fn sale_cost(
        &self,
        world: &World,
        company: u8,
        id: VehicleId,
        backup_order: bool,
        _client_id: u32,
    ) -> Result<CommandCost, CommandError> {
        if !world
            .tables()
            .get(b"VEHS")
            .ok_or(RuntimeError::Invalid("VEHS"))?
            .records()
            .contains_key(&id.raw())
        {
            return Ok(CommandCost::failure("CMD_ERROR"));
        }
        let vehicle = SavedVehicleView::new(world, id)?;
        if vehicle.owner()? != company {
            let mut cost = CommandCost::failure("STR_ERROR_OWNED_BY");
            cost.error_params = vec![0x881D, i64::from(vehicle.owner()?)];
            return Ok(cost);
        }
        let status = vehicle.common_number("vehstatus")?;
        if status & 128 != 0 {
            return Ok(CommandCost::failure("STR_ERROR_VEHICLE_IS_DESTROYED"));
        }
        let tile = vehicle.tile_state()?;
        if status & 2 == 0
            || vehicle.current_speed()? != 0
            || tile.tile_type() >> 4 != 2
            || tile.m5() >> 6 != 2
            || vehicle.road_state()? != 254
        {
            return Ok(CommandCost::failure(
                "STR_ERROR_ROAD_VEHICLE_MUST_BE_STOPPED_INSIDE_DEPOT",
            ));
        }
        admit_destructor(world, vehicle, backup_order)?;
        if !world
            .tables()
            .get(b"BKOR")
            .ok_or(RuntimeError::Invalid("BKOR"))?
            .records()
            .is_empty()
            && self.orders.context() != super::RuntimeSaveContext::SinglePlayer
        {
            return Err(CommandError::Unsupported("sale live backup host context"));
        }
        Ok(CommandCost::success(
            vehicle.common_signed("value")?.saturating_neg(),
            1,
        ))
    }
    pub(crate) fn publish_sale(
        self,
        world: &mut World,
        edits: Vec<WorldEdit>,
        id: VehicleId,
    ) -> Result<(), CommandError> {
        let vehicle = SavedVehicleView::new(world, id)?;
        let mut allocation = self.allocation.clone();
        allocation.pool.free(id.raw()).map_err(RuntimeError::from)?;
        allocation
            .road_units
            .get_mut(&vehicle.owner()?)
            .ok_or(RuntimeError::Invalid("vehicle unit owner"))?
            .release_id(vehicle.unit_number()?)
            .map_err(|_| RuntimeError::Invalid("vehicle unit allocation"))?;
        if !self.road.contains_key(&id) {
            return Err(RuntimeError::Invalid("missing sale cache").into());
        }
        let backups = self.orders.plan_sale_backups(world, id)?;
        let detach = self.orders.plan_detach(world, id)?;
        let mut transaction = world.transaction();
        let pending_backups = backups.stage(&mut transaction)?;
        let pending_orders = detach.stage(&mut transaction)?;
        for edit in edits {
            transaction.apply(edit)?;
        }
        let prepared = transaction.prepare()?;
        if prepared
            .view()
            .table(*b"VEHS")
            .and_then(|t| t.record(id.raw()))
            .is_some()
        {
            return Err(RuntimeError::Invalid("sale candidate still contains vehicle").into());
        }
        let backups = pending_backups.validate(&prepared)?;
        let orders = pending_orders.validate(&prepared)?;
        prepared.commit();
        orders.publish(self.orders);
        backups.publish(self.orders);
        self.road.remove(&id);
        *self.allocation = allocation;
        Ok(())
    }
}
fn admit_destructor(
    world: &World,
    vehicle: SavedVehicleView<'_>,
    backup_order: bool,
) -> Result<(), CommandError> {
    if backup_order || world.tables().get(b"BKOR").is_none() {
        return Err(CommandError::Unsupported("sale order backups"));
    }
    if world
        .tables()
        .get(b"ERNW")
        .is_none_or(|t| !t.records().is_empty())
        || vehicle.common_number("group_id")? != 65534
        || vehicle.common_signed("economy_age")? != 0
        || vehicle.common_signed("profit_this_year")? != 0
        || vehicle.common_signed("profit_last_year")? != 0
    {
        return Err(CommandError::Unsupported(
            "sale group profit or renewal lifecycle",
        ));
    }
    if vehicle.common_number("current_order.type")? & 15 == 3 {
        return Err(CommandError::Unsupported("sale loading lifecycle"));
    }
    if world
        .derived()
        .cargo_payments
        .iter()
        .any(|payment| payment.vehicle == vehicle.id().raw())
    {
        return Err(CommandError::Unsupported("sale cargo payment lifecycle"));
    }
    if vehicle.common_number("last_station_visited")? != 65535 {
        return Err(CommandError::Unsupported("sale station lifecycle"));
    }
    let WireValue::Array(packets) = vehicle.common_field("cargo.packets")? else {
        return Err(RuntimeError::Invalid("cargo packets").into());
    };
    let WireValue::Array(actions) = vehicle.common_field("cargo.action_counts")? else {
        return Err(RuntimeError::Invalid("cargo actions").into());
    };
    if !packets.is_empty() || actions.iter().any(|v| v != &WireValue::Unsigned(0)) {
        return Err(CommandError::Unsupported("sale cargo lifecycle"));
    }
    Ok(())
}
