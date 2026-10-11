//! Final-delivery packet removal, with payment before packet reduction/destruction.
use super::{CargoRuntime, RuntimeError, packet, write};
use crate::{WorldTickError, WorldTickState as State, runtime::VehicleId, unsupported_tick};
use ottd_save::world::WorldEdit;

#[derive(Clone, Copy)]
pub(in crate::runtime) struct Unload {
    pub vehicle: VehicleId,
    pub amount: u32,
}

impl CargoRuntime {
    pub(in crate::runtime) fn keep_delivery(
        &mut self,
        state: &mut State<'_>,
        vehicle: VehicleId,
    ) -> Result<(), WorldTickError> {
        let list = self
            .vehicles
            .get_mut(&vehicle.raw())
            .ok_or(RuntimeError::Invalid("delivery vehicle cargo"))?;
        if list.actions[0] != 0 || list.actions[3] != 0 {
            return Err(unsupported_tick(
                "cargo_unload",
                "transfer or reserved reassignment",
            ));
        }
        list.actions[2] = list.actions[2].wrapping_add(list.actions[1]);
        list.actions[1] = 0;
        write::vehicle(state, vehicle, list)
    }

    pub(in crate::runtime) fn deliver(
        &mut self,
        state: &mut State<'_>,
        unload: Unload,
        mut payment: impl FnMut(&mut State<'_>, u32, u32) -> Result<(), WorldTickError>,
    ) -> Result<u32, WorldTickError> {
        let mut list = self
            .vehicles
            .get(&unload.vehicle.raw())
            .cloned()
            .ok_or(RuntimeError::Invalid("delivery vehicle cargo"))?;
        if list.actions[0] != 0 || list.actions[3] != 0 {
            return Err(unsupported_tick("cargo_unload", "transfer or reservation"));
        }
        let amount = unload.amount.min(list.actions[1]);
        let mut remaining = amount;
        while remaining != 0 {
            let packet = *list
                .packets
                .first()
                .ok_or(RuntimeError::Invalid("delivery packet"))?;
            let count = u32::try_from(state.number(b"CAPA", packet, "count")?)
                .map_err(|_| RuntimeError::Invalid("delivery packet count"))?;
            let moved = count.min(remaining);
            let divisor = std::num::NonZeroI64::new(i64::from(count))
                .ok_or(RuntimeError::Invalid("empty delivery packet"))?;
            let share = state
                .number(b"CAPA", packet, "feeder_share")?
                .saturating_mul(i64::from(moved))
                .checked_div(divisor.get())
                .ok_or(RuntimeError::Invalid("packet feeder division"))?;
            list.count = list.count.wrapping_sub(moved);
            list.periods = list.periods.wrapping_sub(
                state
                    .unsigned(b"CAPA", packet, "periods_in_transit")?
                    .wrapping_mul(u64::from(moved)),
            );
            list.feeder_share = list.feeder_share.saturating_sub(share);
            list.actions[1] = list.actions[1].wrapping_sub(moved);
            payment(state, packet, moved)?;
            if moved == count {
                list.packets.remove(0);
                state.apply(WorldEdit::RemoveRecord {
                    chunk: *b"CAPA",
                    record: packet,
                })?;
                self.pool.free(packet).map_err(RuntimeError::from)?;
            } else {
                packet::reduce(state, packet, moved)?;
            }
            remaining = remaining.wrapping_sub(moved);
        }
        write::vehicle(state, unload.vehicle, &list)?;
        self.vehicles.insert(unload.vehicle.raw(), list);
        Ok(amount)
    }
}
