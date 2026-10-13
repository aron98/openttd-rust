use super::{PreparedWorldTransaction, WorldError, invalid};
use crate::world::OrderListState;
use std::collections::BTreeSet;
/// Native scheduled-order wait removal over an existing shared list cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderDurationDelta {
    /// Complete resident metadata, used to reject stale plans and changed membership.
    pub before: OrderListState,
    /// Sum of removed scheduled wait times, charged once per changed order.
    pub wait_removed: u64,
    /// Removed explicitly timetabled wait contribution.
    pub timetabled_wait_removed: u64,
}
impl PreparedWorldTransaction<'_> {
    /// Validate native deltas against the prepared structural oracle and publish them.
    /// Consumes the candidate so any failure prevents subsequent commit.
    /// # Errors
    /// Rejects stale/duplicate lists, underflow and missing or unrelated cache changes.
    pub fn with_order_duration_deltas(
        mut self,
        deltas: &[OrderDurationDelta],
    ) -> Result<Self, WorldError> {
        let mut seen = BTreeSet::new();
        let mut expected = self.transaction.world.derived.order_lists.clone();
        for delta in deltas {
            if !seen.insert(delta.before.id) {
                return Err(invalid("ORDL", "duplicate duration delta"));
            }
            let entry = expected
                .iter_mut()
                .find(|v| v.id == delta.before.id)
                .ok_or_else(|| invalid("ORDL", "missing duration delta list"))?;
            if *entry != delta.before {
                return Err(invalid("ORDL", "stale duration delta"));
            }
            entry.total_duration = entry
                .total_duration
                .checked_sub(delta.wait_removed)
                .ok_or_else(|| invalid("ORDL", "total duration underflow"))?;
            entry.timetable_duration = entry
                .timetable_duration
                .checked_sub(delta.timetabled_wait_removed)
                .ok_or_else(|| invalid("ORDL", "timetable duration underflow"))?;
        }
        if expected != self.derived.order_lists {
            return Err(invalid(
                "ORDL",
                "candidate differs from native duration delta",
            ));
        }
        self.derived.order_lists = expected;
        Ok(self)
    }
}
