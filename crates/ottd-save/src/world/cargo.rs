use super::derived::{CargoAggregate, Owner};
use super::{Row, TableChunk, WorldError, invalid};
use std::collections::{BTreeMap, BTreeSet};
type Tables = BTreeMap<[u8; 4], TableChunk>;
pub(super) fn aggregate(
    tables: &Tables,
    owner: Owner,
    cargo_type: Option<u32>,
    next_hop: Option<u32>,
    packets: Vec<u32>,
    seen: &mut BTreeSet<u32>,
) -> Result<CargoAggregate, WorldError> {
    let table = tables
        .get(b"CAPA")
        .ok_or_else(|| invalid("CAPA", "missing packet table"))?;
    let mut value = CargoAggregate {
        owner,
        cargo_type,
        next_hop,
        packets: Vec::new(),
        count: 0,
        periods_in_transit: 0,
        feeder_share: 0,
    };
    for id in &packets {
        if !seen.insert(*id) {
            return Err(invalid("CAPA", "packet belongs to multiple lists"));
        }
        let record = table
            .records()
            .get(id)
            .ok_or_else(|| invalid("CAPA", "missing cargo packet"))?;
        let row = Row {
            schema: table.schema(),
            record,
        };
        let count = u32::try_from(row.unsigned("count")?)
            .map_err(|_| invalid("CAPA/count", "count overflow"))?;
        value.accumulate(
            count,
            row.unsigned("periods_in_transit")?,
            row.signed("feeder_share")?,
        );
    }
    value.packets = packets;
    Ok(value)
}

impl CargoAggregate {
    fn accumulate(&mut self, count: u32, periods: u64, feeder_share: i64) {
        self.count = self.count.wrapping_add(count);
        self.periods_in_transit = self
            .periods_in_transit
            .wrapping_add(periods.wrapping_mul(u64::from(count)));
        self.feeder_share = self.feeder_share.saturating_add(feeder_share);
    }
}

#[cfg(test)]
mod tests {
    use super::{CargoAggregate, Owner};
    use crate::world::Pool;
    #[test]
    fn cargo_cache_arithmetic_matches_native_boundaries() {
        let mut aggregate = CargoAggregate {
            owner: Owner {
                pool: Pool::Vehicle,
                id: 0,
            },
            cargo_type: None,
            next_hop: None,
            packets: Vec::new(),
            count: u32::MAX,
            periods_in_transit: u64::MAX,
            feeder_share: i64::MIN,
        };
        aggregate.accumulate(1, 2, -1);
        assert_eq!(
            (
                aggregate.count,
                aggregate.periods_in_transit,
                aggregate.feeder_share
            ),
            (0, 1, i64::MIN)
        );
    }
}
