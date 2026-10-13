use super::{
    BackupAllocation, OrderState, RuntimeError,
    view::{self, OrderReader, Row},
};
use ottd_save::{
    WireValue,
    world::{
        OrderDurationDelta, OrderListState, PathElement, PreparedWorldTransaction, WorldEdit,
        WorldTransaction,
    },
};
use std::collections::BTreeSet;
/// Sparse saved mutations and tentative native allocation; no publication yet.
#[derive(Debug)]
pub(in crate::runtime) struct OrderPlan {
    edits: Vec<WorldEdit>,
    pending: PendingOrderDelta,
}
#[derive(Debug)]
pub(in crate::runtime) struct PendingOrderDelta {
    durations: Vec<OrderDurationDelta>,
    backups: BackupAllocation,
}
#[derive(Debug)]
pub(in crate::runtime) struct PreparedOrderDelta {
    backups: BackupAllocation,
}
impl OrderPlan {
    pub(in crate::runtime) fn stage(
        self,
        tx: &mut WorldTransaction<'_>,
    ) -> Result<PendingOrderDelta, RuntimeError> {
        for edit in self.edits {
            tx.apply(edit)?;
        }
        Ok(self.pending)
    }
}
impl PendingOrderDelta {
    pub(in crate::runtime) fn validate(
        self,
        prepared: PreparedWorldTransaction<'_>,
    ) -> Result<(PreparedWorldTransaction<'_>, PreparedOrderDelta), RuntimeError> {
        let ids: Vec<_> = prepared
            .view()
            .table(*b"BKOR")
            .ok_or(RuntimeError::Invalid("BKOR"))?
            .records()
            .map(|(id, _)| id)
            .collect();
        self.backups.validate_slots(&ids)?;
        let prepared = prepared.with_order_duration_deltas(&self.durations)?;
        Ok((
            prepared,
            PreparedOrderDelta {
                backups: self.backups,
            },
        ))
    }
}
impl PreparedOrderDelta {
    pub(in crate::runtime) fn publish(self, state: &mut OrderState) {
        state.backups = self.backups;
    }
}
const fn field(chunk: [u8; 4], id: u32, path: Vec<PathElement>, value: u64) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record: id,
        path,
        value: WireValue::Unsigned(value),
    }
}
fn current(id: u32, variant: &str, name: &str, value: u64) -> WorldEdit {
    use PathElement::{Field, Index};
    field(
        *b"VEHS",
        id,
        vec![
            Field(variant.into()),
            Index(0),
            Field("common".into()),
            Index(0),
            Field(name.into()),
        ],
        value,
    )
}
fn scheduled(id: u32, index: usize, name: &str, value: u64) -> WorldEdit {
    use PathElement::{Field, Index};
    field(
        *b"ORDL",
        id,
        vec![Field("orders".into()), Index(index), Field(name.into())],
        value,
    )
}
fn matches(order: Row<'_>, depot: u16) -> Result<bool, RuntimeError> {
    Ok(order.number("type")? & 15 == 2
        && order.number("dest")? == u64::from(depot)
        && order.number("flags")? & 16 == 0)
}
impl OrderState {
    pub(in crate::runtime) fn plan_depot_invalidation(
        &self,
        reader: OrderReader<'_>,
        derived: &[OrderListState],
        depot: u16,
        tile: u32,
        width: u32,
        height: u32,
    ) -> Result<OrderPlan, RuntimeError> {
        let mut edits = Vec::new();
        let mut durations = Vec::new();
        let mut seen = BTreeSet::new();
        let lists = reader.rows(*b"ORDL")?;
        for (id, row) in reader.rows(*b"VEHS")? {
            let Some((variant, common)) = view::vehicle(row)? else {
                continue;
            };
            if variant != "aircraft"
                && common.number("current_order.type")? & 15 == 2
                && common.number("current_order.dest")? == u64::from(depot)
            {
                edits.push(current(id, variant, "current_order.type", 5));
                edits.push(current(id, variant, "current_order.flags", 0));
            }
            let reference = common.number("orders")?;
            if reference == 0 || variant == "aircraft" {
                continue;
            }
            let list_id = u32::try_from(reference.saturating_sub(1))
                .map_err(|_| RuntimeError::Invalid("order list reference"))?;
            if !seen.insert(list_id) {
                continue;
            }
            let row = lists
                .iter()
                .find(|(id, _)| *id == list_id)
                .ok_or(RuntimeError::Invalid("shared order list"))?
                .1;
            let before = derived
                .iter()
                .find(|v| v.id == list_id)
                .ok_or(RuntimeError::Invalid("shared order metadata"))?;
            let mut delta = OrderDurationDelta {
                before: before.clone(),
                wait_removed: 0,
                timetabled_wait_removed: 0,
            };
            let (schema, orders) = row.children("orders")?;
            for (index, record) in orders.iter().enumerate() {
                let order = Row { schema, record };
                if !matches(order, depot)? {
                    continue;
                }
                let wait = order.number("wait_time")?;
                let flags = order.number("flags")?;
                delta.wait_removed = delta
                    .wait_removed
                    .checked_add(wait)
                    .ok_or(RuntimeError::Invalid("order wait sum"))?;
                if flags & 8 != 0 {
                    delta.timetabled_wait_removed = delta
                        .timetabled_wait_removed
                        .checked_add(wait)
                        .ok_or(RuntimeError::Invalid("order timetable sum"))?;
                }
                edits.push(scheduled(list_id, index, "type", 5));
                edits.push(scheduled(list_id, index, "flags", flags & 128));
                edits.push(scheduled(list_id, index, "wait_time", 0));
            }
            durations.push(delta);
        }
        let mut backups = self.backups.clone();
        for (id, row) in reader.rows(*b"BKOR")? {
            let backup_tile = u32::try_from(row.number("tile")?)
                .map_err(|_| RuntimeError::Invalid("backup tile"))?;
            let mut remove = backup_tile == tile;
            if !remove {
                let (schema, orders) = row.children("orders")?;
                for record in orders {
                    let order = Row { schema, record };
                    if !matches(order, depot)? {
                        continue;
                    }
                    if super::airport::is_hangar(reader, backup_tile, width, height)? {
                        continue;
                    }
                    remove = true;
                    break;
                }
            }
            if remove {
                backups.free(id)?;
                edits.push(WorldEdit::RemoveRecord {
                    chunk: *b"BKOR",
                    record: id,
                });
            }
        }
        Ok(OrderPlan {
            edits,
            pending: PendingOrderDelta { durations, backups },
        })
    }
}
