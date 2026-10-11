//! Ordinary manual-flow arrival partition, preserving packet identity and order.
use super::{CargoRuntime, RuntimeError, write};
use crate::{WorldTickError, WorldTickState as State, runtime::VehicleId, unsupported_tick};

#[derive(Clone, Copy)]
pub(in crate::runtime) struct Arrival {
    pub vehicle: VehicleId,
    pub station: u32,
    pub cargo: u8,
}

impl CargoRuntime {
    pub(in crate::runtime) fn stage(
        &mut self,
        state: &mut State<'_>,
        arrival: Arrival,
    ) -> Result<bool, WorldTickError> {
        let goods = write::goods(state, arrival.station, arrival.cargo)?;
        if !goods.rows("flow")?.1.is_empty() {
            return Err(unsupported_tick("cargo_stage", "flow routing and transfer"));
        }
        let accepted = goods.number("status")? & 1 != 0;
        let mut list = self
            .vehicles
            .get(&arrival.vehicle.raw())
            .cloned()
            .ok_or(RuntimeError::Invalid("staged vehicle cargo"))?;
        if list.actions[3] != 0 {
            return Err(unsupported_tick("cargo_stage", "reserved cargo on arrival"));
        }
        list.actions = [0; 4];
        let mut delivered = Vec::new();
        let mut kept = Vec::new();
        for packet in &list.packets {
            let count = u32::try_from(state.number(b"CAPA", *packet, "count")?)
                .map_err(|_| RuntimeError::Invalid("staged packet count"))?;
            if accepted && state.number(b"CAPA", *packet, "source")? != i64::from(arrival.station) {
                delivered.push(*packet);
                list.actions[1] = list.actions[1].wrapping_add(count);
            } else {
                kept.push(*packet);
                list.actions[2] = list.actions[2].wrapping_add(count);
            }
        }
        delivered.extend(kept);
        list.packets = delivered;
        let unload = list.actions[1] > 0;
        write::vehicle(state, arrival.vehicle, &list)?;
        self.vehicles.insert(arrival.vehicle.raw(), list);
        Ok(unload)
    }
}
