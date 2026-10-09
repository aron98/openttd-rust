use super::derived::{DerivedState, OrderListState, add};
use super::{TableChunk, WorldError, invalid, rows};
use std::collections::BTreeMap;
type Tables = BTreeMap<[u8; 4], TableChunk>;
pub(super) fn restore_orders(
    tables: &Tables,
    order_ids: &BTreeMap<u32, Option<u32>>,
    shared: &BTreeMap<u32, Option<u32>>,
    previous_shared: &BTreeMap<u32, u32>,
    state: &mut DerivedState,
) -> Result<(), WorldError> {
    for (id, row) in rows(tables, *b"ORDL")? {
        let members: Vec<_> = order_ids
            .iter()
            .filter_map(|(vehicle, list)| (*list == Some(id)).then_some(*vehicle))
            .collect();
        let roots: Vec<_> = members
            .iter()
            .filter(|id| !previous_shared.contains_key(id))
            .copied()
            .collect();
        if !members.is_empty() && roots.len() != 1 {
            return Err(invalid(
                "ORDL",
                "order list must have one shared-chain head",
            ));
        }
        let first_shared = roots.first().copied();
        let mut vehicles = Vec::new();
        let mut current = first_shared;
        while let Some(vehicle) = current {
            if order_ids.get(&vehicle) != Some(&Some(id)) {
                return Err(invalid(
                    "VEHS/next_shared",
                    "shared vehicles have different order lists",
                ));
            }
            vehicles.push(vehicle);
            current = shared.get(&vehicle).copied().flatten();
        }
        if vehicles.len() != members.len() {
            return Err(invalid("ORDL", "disconnected shared-order membership"));
        }
        let mut value = OrderListState {
            id,
            first_shared,
            vehicles,
            num_manual_orders: 0,
            total_duration: 0,
            timetable_duration: 0,
        };
        for order in row.child("orders")? {
            let kind = order.unsigned("type")? & 15;
            if kind > 8 {
                return Err(invalid("ORDL/orders/type", "unknown order type"));
            }
            if kind != 8 {
                value.num_manual_orders = value
                    .num_manual_orders
                    .checked_add(1)
                    .ok_or_else(|| invalid("ORDL", "order count overflow"))?;
            }
            let wait = order.unsigned("wait_time")?;
            let travel = order.unsigned("travel_time")?;
            let flags = order.unsigned("flags")?;
            value.total_duration = add(value.total_duration, add(wait, travel)?)?;
            if kind == 7 || flags & 8 != 0 {
                value.timetable_duration = add(value.timetable_duration, wait)?;
            }
            if kind == 7 || flags & 128 != 0 {
                value.timetable_duration = add(value.timetable_duration, travel)?;
            }
        }
        state.order_lists.push(value);
    }
    Ok(())
}
