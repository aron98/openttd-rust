//! Native randomized station truncation with source-station waiting feedback.
use super::{CargoRuntime, RuntimeError, write};
use crate::{WorldTickError, WorldTickState as State, unsupported_tick as unsupported};
use ottd_save::world::WorldEdit;
use std::collections::BTreeMap;

pub(super) fn random(state: &mut State<'_>) -> Result<u32, WorldTickError> {
    let mut rng = ottd_core::Randomizer::from_state(state.random_state()?);
    let value = rng.next_u32();
    for (name, word) in ["random_state[0]", "random_state[1]"]
        .into_iter()
        .zip(rng.state())
    {
        state.set_number(b"DATE", 0, name, i64::from(word))?;
    }
    Ok(value)
}
impl CargoRuntime {
    pub(super) fn truncate(
        &mut self,
        state: &mut State<'_>,
        station: u32,
        cargo: u8,
        amount: u32,
    ) -> Result<(), WorldTickError> {
        let Some(mut list) = self
            .stations
            .get(&station)
            .and_then(|goods| goods.get(&u32::from(cargo)))
            .cloned()
        else {
            return Ok(());
        };
        let limit = amount.min(list.count);
        let previous = list.count;
        let mut moved = 0_u32;
        let mut round = 0_u32;
        let mut by_source = BTreeMap::<u32, u32>::new();
        'outer: while moved < limit {
            let packets = list
                .packets
                .iter()
                .flat_map(|(next, ids)| ids.iter().map(|id| (*next, *id)))
                .collect::<Vec<_>>();
            for (next, id) in packets {
                let count = u32::try_from(state.number(b"CAPA", id, "count")?)
                    .map_err(|_| unsupported("cargo_truncate", "count"))?;
                let source = u32::try_from(state.number(b"CAPA", id, "source")?)
                    .map_err(|_| unsupported("cargo_truncate", "first station"))?;
                if previous > limit
                    && (u64::from(random(state)?).wrapping_mul(u64::from(previous)) >> 32)
                        < u64::from(previous.wrapping_sub(limit))
                {
                    if round == 0 {
                        let entry = by_source.entry(source).or_default();
                        *entry = entry.wrapping_add(count);
                    }
                    continue;
                }
                let remaining = limit.wrapping_sub(moved);
                let removed = count.min(remaining);
                let periods = state
                    .unsigned(b"CAPA", id, "periods_in_transit")?
                    .wrapping_mul(u64::from(removed));
                list.count = list
                    .count
                    .checked_sub(removed)
                    .ok_or(RuntimeError::Invalid("truncated list count"))?;
                list.periods = list
                    .periods
                    .checked_sub(periods)
                    .ok_or(RuntimeError::Invalid("truncated list periods"))?;
                moved = moved.wrapping_add(removed);
                if count > remaining {
                    if removed != 0 {
                        super::packet::reduce(state, id, removed)?;
                    }
                    let entry = by_source.entry(source).or_default();
                    if round > 0 {
                        *entry = entry.wrapping_sub(removed);
                        break 'outer;
                    }
                    *entry = entry.wrapping_add(count.wrapping_sub(removed));
                } else {
                    let ids = list
                        .packets
                        .get_mut(&next)
                        .ok_or(RuntimeError::Invalid("truncate packet group"))?;
                    ids.retain(|packet| *packet != id);
                    if ids.is_empty() {
                        list.packets.remove(&next);
                    }
                    if round > 0 {
                        let entry = by_source.entry(source).or_default();
                        *entry = entry.wrapping_sub(count);
                    }
                    state.apply(WorldEdit::RemoveRecord {
                        chunk: *b"CAPA",
                        record: id,
                    })?;
                    self.pool.free(id).map_err(RuntimeError::from)?;
                }
            }
            round = round.wrapping_add(1);
        }
        write::station(state, station, cargo, &list)?;
        self.stations
            .get_mut(&station)
            .ok_or(RuntimeError::Invalid("truncated station cache"))?
            .insert(u32::from(cargo), list);
        for (source, count) in by_source {
            if state.table(b"STNN")?.record(source).is_some() {
                let old = write::goods(state, source, cargo)?.number("max_waiting_cargo")?;
                write::goods_field(
                    state,
                    source,
                    cargo,
                    "max_waiting_cargo",
                    old.max(u64::from(count)),
                )?;
            }
        }
        Ok(())
    }
}
