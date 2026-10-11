//! Native unreserved station-list loading in next-stop order, then `INVALID_STATION`.
use super::{CargoRuntime, RuntimeError, packet, write};
use crate::{
    WorldTickError, WorldTickState as State, runtime::VehicleId, unsupported_tick as unsupported,
};

#[derive(Clone, Copy)]
pub(in crate::runtime) struct Load<'a> {
    pub station: u32,
    pub cargo: u8,
    pub vehicle: VehicleId,
    pub amount: u32,
    pub tile: u32,
    pub next: &'a [u16],
}
impl CargoRuntime {
    pub(in crate::runtime) fn load(
        &mut self,
        state: &mut State<'_>,
        load: Load<'_>,
    ) -> Result<u32, WorldTickError> {
        let mut vehicle = self
            .vehicles
            .get(&load.vehicle.raw())
            .cloned()
            .ok_or(RuntimeError::Invalid("cargo vehicle list"))?;
        if vehicle.actions.get(3).copied() != Some(0) {
            return Err(unsupported("cargo_load", "reserved cargo reassignment"));
        }
        let mut station = self
            .stations
            .get(&load.station)
            .and_then(|goods| goods.get(&u32::from(load.cargo)))
            .cloned()
            .unwrap_or_default();
        let mut remaining = load.amount;
        for next in load
            .next
            .iter()
            .rev()
            .copied()
            .chain(std::iter::once(u16::MAX))
        {
            while remaining != 0 {
                let Some(id) = station
                    .packets
                    .get(&u32::from(next))
                    .and_then(|ids| ids.first())
                    .copied()
                else {
                    break;
                };
                let count = u32::try_from(state.number(b"CAPA", id, "count")?)
                    .map_err(|_| unsupported("cargo_load", "packet count"))?;
                let partial = remaining < count;
                let moved = count.min(remaining);
                let packet = if partial {
                    let split = packet::split(
                        state,
                        self,
                        id,
                        u16::try_from(moved)
                            .map_err(|_| unsupported("cargo_load", "split count"))?,
                    )?;
                    remaining = 0;
                    let Some(split) = split else { break };
                    split
                } else {
                    remaining = remaining.wrapping_sub(count);
                    let ids = station
                        .packets
                        .get_mut(&u32::from(next))
                        .ok_or(RuntimeError::Invalid("cargo next-hop list"))?;
                    ids.remove(0);
                    if ids.is_empty() {
                        station.packets.remove(&u32::from(next));
                    }
                    id
                };
                packet::loading_tile(state, packet, load.tile)?;
                let periods = state
                    .unsigned(b"CAPA", packet, "periods_in_transit")?
                    .wrapping_mul(u64::from(moved));
                station.count = station
                    .count
                    .checked_sub(moved)
                    .ok_or(RuntimeError::Invalid("station cargo count"))?;
                station.periods = station
                    .periods
                    .checked_sub(periods)
                    .ok_or(RuntimeError::Invalid("station cargo periods"))?;
                append(state, self, &mut vehicle, packet, moved, periods)?;
                if partial {
                    break;
                }
            }
            if remaining == 0 {
                break;
            }
        }
        write::station(state, load.station, load.cargo, &station)?;
        write::vehicle(state, load.vehicle, &vehicle)?;
        self.stations
            .get_mut(&load.station)
            .ok_or(RuntimeError::Invalid("station cargo cache"))?
            .insert(u32::from(load.cargo), station);
        self.vehicles.insert(load.vehicle.raw(), vehicle);
        Ok(load.amount.wrapping_sub(remaining))
    }
}

fn append(
    state: &mut State<'_>,
    cargo: &mut CargoRuntime,
    vehicle: &mut super::VehicleList,
    packet: u32,
    moved: u32,
    periods: u64,
) -> Result<(), WorldTickError> {
    let feeder = state.number(b"CAPA", packet, "feeder_share")?;
    vehicle.count = vehicle.count.wrapping_add(moved);
    vehicle.periods = vehicle.periods.wrapping_add(periods);
    vehicle.feeder_share = vehicle.feeder_share.saturating_add(feeder);
    let kept = vehicle
        .actions
        .get_mut(2)
        .ok_or(RuntimeError::Invalid("vehicle keep action"))?;
    *kept = kept.wrapping_add(moved);
    let mut sum = moved;
    let mut merged = false;
    for existing in vehicle.packets.iter().rev() {
        if packet::merge(state, cargo, *existing, packet)? {
            merged = true;
            break;
        }
        sum = sum.wrapping_add(
            u32::try_from(state.number(b"CAPA", *existing, "count")?)
                .map_err(|_| unsupported("cargo_load", "packet count"))?,
        );
        if sum >= *kept {
            break;
        }
    }
    if !merged {
        vehicle.packets.push(packet);
    }
    Ok(())
}
