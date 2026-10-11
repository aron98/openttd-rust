//! Sorted native `BaseNode` edge insertion and `BaseEdge` refresh/increase arithmetic.
use super::{Traversal, Update, WireValue, WorldTickError, records, unsupported};
use crate::runtime::stations::saved::StationRow;
use ottd_save::{TableRecord, TableSchema};
#[derive(Clone, Copy)]
pub(super) struct Observation<'a> {
    pub destination: u64,
    pub date: i64,
    pub link: &'a Traversal,
}
pub(super) fn update(
    schema: &TableSchema,
    edges: &mut Vec<TableRecord>,
    observation: Observation<'_>,
) -> Result<(), WorldTickError> {
    let destination = observation.destination;
    let position = edges.iter().position(|record| {
        (StationRow { schema, record })
            .number("dest_node")
            .is_ok_and(|id| id >= destination)
    });
    let existing = if let Some(position) = position {
        (StationRow {
            schema,
            record: edges
                .get(position)
                .ok_or_else(|| unsupported("links", "edge position"))?,
        })
        .number("dest_node")?
            == destination
    } else {
        false
    };
    if !existing {
        let record = new_edge(schema, observation)?;
        edges.insert(position.unwrap_or(edges.len()), record);
        return Ok(());
    }
    let record = edges
        .get_mut(position.ok_or_else(|| unsupported("links", "edge position"))?)
        .ok_or_else(|| unsupported("links", "edge position"))?;
    update_existing(schema, record, observation)
}
fn new_edge(
    schema: &TableSchema,
    observation: Observation<'_>,
) -> Result<TableRecord, WorldTickError> {
    let Observation {
        destination,
        date,
        link,
    } = observation;
    records::construct(
        schema,
        &[
            ("dest_node", WireValue::Unsigned(destination)),
            ("capacity", WireValue::Unsigned(u64::from(link.capacity))),
            ("usage", WireValue::Unsigned(0)),
            (
                "travel_time_sum",
                WireValue::Unsigned(u64::from(link.capacity).wrapping_mul(u64::from(link.time))),
            ),
            (
                "last_unrestricted_update",
                WireValue::Signed(match link.mode {
                    Update::Refresh => date,
                    Update::Increase => -1,
                }),
            ),
            ("last_restricted_update", WireValue::Signed(-1)),
        ],
    )
}
fn update_existing(
    schema: &TableSchema,
    record: &mut TableRecord,
    observation: Observation<'_>,
) -> Result<(), WorldTickError> {
    let Observation { date, link, .. } = observation;
    let row = StationRow { schema, record };
    let old = u32::try_from(row.number("capacity")?)
        .map_err(|_| unsupported("links", "edge capacity"))?;
    let divisor = std::num::NonZeroU64::new(u64::from(old))
        .ok_or_else(|| unsupported("links", "zero edge capacity"))?;
    let sum = row.number("travel_time_sum")?;
    let incoming = u64::from(link.capacity);
    let (capacity, sum) = match link.mode {
        Update::Increase => (
            old.wrapping_add(link.capacity),
            if sum == 0 {
                u64::from(old.wrapping_add(link.capacity)).wrapping_mul(u64::from(link.time))
            } else if link.time == 0 {
                sum.wrapping_add((sum / divisor).wrapping_mul(incoming))
            } else {
                sum.wrapping_add(u64::from(link.time).wrapping_mul(incoming))
            },
        ),
        Update::Refresh => {
            if sum == 0 {
                (
                    old.max(link.capacity),
                    u64::from(old.max(link.capacity)).wrapping_mul(u64::from(link.time)),
                )
            } else if link.capacity > old {
                (link.capacity, (sum / divisor).wrapping_mul(incoming))
            } else {
                (old, sum)
            }
        }
    };
    records::edit(
        schema,
        record,
        ("capacity", WireValue::Unsigned(u64::from(capacity))),
    )?;
    records::edit(
        schema,
        record,
        ("travel_time_sum", WireValue::Unsigned(sum)),
    )?;
    if matches!(link.mode, Update::Refresh) {
        records::edit(
            schema,
            record,
            ("last_unrestricted_update", WireValue::Signed(date)),
        )?;
    }
    Ok(())
}
