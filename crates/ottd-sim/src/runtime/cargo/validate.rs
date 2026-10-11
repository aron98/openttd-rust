//! Independently verify cached list metadata against candidate packet rows and references.
use super::{CargoRuntime, RuntimeError, read::Read};
use crate::runtime::stations::saved::StationRow;
use ottd_save::{WireValue, world::CandidateView};
use std::collections::{BTreeMap, BTreeSet};

fn value<'a>(row: StationRow<'a>, name: &str) -> Result<&'a WireValue, RuntimeError> {
    crate::world_access::row_field(row.schema, row.record, name)
        .map_err(|_| RuntimeError::Invalid("cargo candidate field"))
}
fn array(row: StationRow<'_>, name: &str) -> Result<Vec<u32>, RuntimeError> {
    let WireValue::Array(values) = value(row, name)? else {
        return Err(RuntimeError::Invalid("cargo candidate array"));
    };
    values
        .iter()
        .map(|value| match value {
            WireValue::Unsigned(value) => {
                u32::try_from(*value).map_err(|_| RuntimeError::Invalid("cargo candidate number"))
            }
            _ => Err(RuntimeError::Invalid("cargo candidate number type")),
        })
        .collect()
}
fn pointers(row: StationRow<'_>, name: &str) -> Result<Vec<u32>, RuntimeError> {
    array(row, name)?
        .into_iter()
        .map(|pointer| {
            pointer
                .checked_sub(1)
                .ok_or(RuntimeError::Invalid("null cargo pointer"))
        })
        .collect()
}
fn totals(
    view: Read<'_>,
    packets: &[u32],
    seen: &mut BTreeSet<u32>,
) -> Result<(u32, u64, i64), RuntimeError> {
    let mut count = 0_u32;
    let mut periods = 0_u64;
    let mut feeder = 0_i64;
    for id in packets {
        if !seen.insert(*id) {
            return Err(RuntimeError::Invalid(
                "packet appears in multiple cargo lists",
            ));
        }
        let row = view.row(*b"CAPA", *id)?;
        let items = u32::try_from(row.number("count")?)
            .map_err(|_| RuntimeError::Invalid("cached packet count"))?;
        count = count.wrapping_add(items);
        periods = periods.wrapping_add(
            row.number("periods_in_transit")?
                .wrapping_mul(u64::from(items)),
        );
        let WireValue::Signed(share) = value(row, "feeder_share")? else {
            return Err(RuntimeError::Invalid("packet feeder share"));
        };
        feeder = feeder.wrapping_add(*share);
    }
    Ok((count, periods, feeder))
}
pub(super) fn lists(cargo: &CargoRuntime, view: CandidateView<'_>) -> Result<(), RuntimeError> {
    let view = Read::Candidate(view);
    let mut seen = BTreeSet::new();
    for (id, list) in &cargo.vehicles {
        let root = view.row(*b"VEHS", *id)?;
        let mut common = None;
        for kind in ["train", "roadveh", "ship", "aircraft"] {
            if !root.rows(kind)?.1.is_empty() {
                common = Some(root.child(kind)?.child("common")?);
                break;
            }
        }
        let common = common.ok_or(RuntimeError::Invalid("cargo vehicle kind"))?;
        if pointers(common, "cargo.packets")? != list.packets
            || array(common, "cargo.action_counts")? != list.actions
        {
            return Err(RuntimeError::Invalid("cargo vehicle saved list"));
        }
        let (count, periods, feeder) = totals(view, &list.packets, &mut seen)?;
        if count != list.count
            || periods != list.periods
            || feeder != list.feeder_share
            || list
                .actions
                .iter()
                .try_fold(0_u32, |sum, value| sum.checked_add(*value))
                != Some(count)
        {
            return Err(RuntimeError::Invalid("cargo vehicle cached totals"));
        }
    }
    for (station, caches) in &cargo.stations {
        let (schema, goods) = view
            .row(*b"STNN", *station)?
            .child("normal")?
            .rows("goods")?;
        for (id, record) in goods.iter().enumerate() {
            let id = u32::try_from(id).map_err(|_| RuntimeError::Invalid("cargo id"))?;
            let row = StationRow { schema, record };
            let (packet_schema, groups) = row.rows("cargo")?;
            let mut packets = BTreeMap::new();
            for record in groups {
                let group = StationRow {
                    schema: packet_schema,
                    record,
                };
                let next = u32::try_from(group.number("first")?)
                    .map_err(|_| RuntimeError::Invalid("cargo next hop"))?;
                if packets.insert(next, pointers(group, "second")?).is_some() {
                    return Err(RuntimeError::Invalid("duplicate cargo next hop"));
                }
            }
            let reserved = u32::try_from(row.number("cargo.reserved_count")?)
                .map_err(|_| RuntimeError::Invalid("reserved cargo"))?;
            if let Some(cache) = caches.get(&id) {
                if packets != cache.packets || reserved != cache.reserved {
                    return Err(RuntimeError::Invalid("station saved cargo list"));
                }
                let ids = packets.values().flatten().copied().collect::<Vec<_>>();
                let (count, periods, _) = totals(view, &ids, &mut seen)?;
                if count != cache.count || periods != cache.periods {
                    return Err(RuntimeError::Invalid("station cached cargo totals"));
                }
            } else if !packets.is_empty() || reserved != 0 || !row.rows("flow")?.1.is_empty() {
                return Err(RuntimeError::Invalid("station cargo data cache missing"));
            }
        }
    }
    Ok(())
}
