use super::{TableChunk, WorldError, invalid, references, rows};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

type Tables = BTreeMap<[u8; 4], TableChunk>;

/// The pool of a structural ownership relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Pool {
    /// Vehicles.
    #[serde(rename = "VEHS")]
    Vehicle,
    /// Stations.
    #[serde(rename = "STNN")]
    Station,
    /// Towns.
    #[serde(rename = "CITY")]
    Town,
    /// Industries.
    #[serde(rename = "INDY")]
    Industry,
}
/// Stable pool-qualified object identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Owner {
    /// Pool identity.
    pub pool: Pool,
    /// Native sparse pool index, not an encoded reference.
    pub id: u32,
}
/// Reconstructed vehicle links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VehicleLinks {
    /// Vehicle ID.
    pub id: u32,
    /// Previous consist member.
    pub previous: Option<u32>,
    /// First consist member.
    pub first: u32,
    /// Previous shared-order vehicle.
    pub previous_shared: Option<u32>,
}
/// Reconstructed order-list membership and timing caches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OrderListState {
    /// Order-list ID.
    pub id: u32,
    /// Head of the shared vehicle chain.
    pub first_shared: Option<u32>,
    /// Vehicle IDs in shared-chain order.
    pub vehicles: Vec<u32>,
    /// Number of non-implicit orders.
    pub num_manual_orders: u32,
    /// Sum of all wait and travel durations.
    pub total_duration: u64,
    /// Sum of explicitly timetabled wait and travel durations.
    pub timetable_duration: u64,
}
/// Aggregate reconstructed from exactly one packet list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CargoAggregate {
    /// Owning station or vehicle.
    pub owner: Owner,
    /// Goods slot for stations; absent for vehicle cargo.
    pub cargo_type: Option<u32>,
    /// Destination group for station packets; absent for vehicle cargo.
    pub next_hop: Option<u32>,
    /// Packet IDs in original list order.
    pub packets: Vec<u32>,
    /// Total units.
    pub count: u32,
    /// Sum of each packet's count times transit periods.
    pub periods_in_transit: u64,
    /// Sum of packet feeder shares.
    pub feeder_share: i64,
}
/// Direct child IDs of a group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GroupChildren {
    /// Group ID.
    pub id: u32,
    /// Child IDs sorted in pool order.
    pub children: Vec<u32>,
}
/// Reconstructed station roadstop chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoadStopChain {
    /// Station ID.
    pub station: u32,
    /// Bus or truck facility.
    pub kind: &'static str,
    /// Roadstop IDs in chain order.
    pub stops: Vec<u32>,
}
/// Persistent storage owner binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StorageOwner {
    /// Persistent-storage ID.
    pub id: u32,
    /// Town, station or industry that owns the storage.
    pub owner: Owner,
}
/// Cargo-payment reverse vehicle binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CargoPayment {
    /// Cargo-payment pool ID.
    pub id: u32,
    /// Front vehicle ID.
    pub vehicle: u32,
}
/// Explicit content-independent after-load state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DerivedState {
    /// Vehicle reverse and first links.
    pub vehicles: Vec<VehicleLinks>,
    /// Shared-order membership and timing aggregates.
    pub order_lists: Vec<OrderListState>,
    /// Cargo ownership and packet aggregates.
    pub cargo_lists: Vec<CargoAggregate>,
    /// Direct group children.
    pub groups: Vec<GroupChildren>,
    /// Station-owned roadstop chains.
    pub road_stop_chains: Vec<RoadStopChain>,
    /// Persistent storage owners.
    pub storage_owners: Vec<StorageOwner>,
    /// Cargo payment bindings.
    pub cargo_payments: Vec<CargoPayment>,
}

pub(super) fn restore(tables: &Tables) -> Result<DerivedState, WorldError> {
    let mut state = DerivedState::default();
    let mut next = BTreeMap::new();
    let mut shared = BTreeMap::new();
    let mut order_ids = BTreeMap::new();
    let mut cargo_seen = BTreeSet::new();
    for (id, row) in rows(tables, *b"VEHS")? {
        let (kind, common) = references::vehicle_row(row)?;
        next.insert(
            id,
            common.map(|r| r.reference("next")).transpose()?.flatten(),
        );
        if kind < 4 {
            let common = common.ok_or_else(|| invalid("VEHS", "missing common state"))?;
            shared.insert(id, common.reference("next_shared")?);
            order_ids.insert(id, common.reference("orders")?);
            let packets = common.references("cargo.packets")?;
            let aggregate = super::cargo::aggregate(
                tables,
                Owner {
                    pool: Pool::Vehicle,
                    id,
                },
                None,
                None,
                packets,
                &mut cargo_seen,
            )?;
            let super::WireValue::Array(actions) = common.value("cargo.action_counts")? else {
                return Err(invalid("VEHS/cargo.action_counts", "invalid counts"));
            };
            let mut total = 0u32;
            for action in actions {
                let super::WireValue::Unsigned(count) = action else {
                    return Err(invalid("VEHS/cargo.action_counts", "invalid count"));
                };
                total = total.wrapping_add(
                    u32::try_from(*count)
                        .map_err(|_| invalid("cargo.action_counts", "count overflow"))?,
                );
            }
            if total != aggregate.count {
                return Err(invalid(
                    "VEHS/cargo.action_counts",
                    "counts differ from cargo packets",
                ));
            }
            state.cargo_lists.push(aggregate);
        } else {
            shared.insert(id, None);
        }
    }
    let previous: BTreeMap<_, _> = next
        .iter()
        .filter_map(|(id, n)| n.map(|n| (n, *id)))
        .collect();
    let previous_shared: BTreeMap<_, _> = shared
        .iter()
        .filter_map(|(id, n)| n.map(|n| (n, *id)))
        .collect();
    let mut firsts = BTreeMap::new();
    for head in next.keys().filter(|id| !previous.contains_key(id)) {
        let mut current = Some(*head);
        while let Some(id) = current {
            firsts.insert(id, *head);
            current = next.get(&id).copied().flatten();
        }
    }
    for id in next.keys() {
        state.vehicles.push(VehicleLinks {
            id: *id,
            previous: previous.get(id).copied(),
            first: *firsts
                .get(id)
                .ok_or_else(|| invalid("VEHS", "unrooted consist"))?,
            previous_shared: previous_shared.get(id).copied(),
        });
    }
    super::orders::restore_orders(tables, &order_ids, &shared, &previous_shared, &mut state)?;
    restore_station_cargo(tables, &mut state, &mut cargo_seen)?;
    super::ownership::restore_groups(tables, &mut state)?;
    super::ownership::restore_ownership(tables, &mut state)?;
    Ok(state)
}
pub(super) fn add(a: u64, b: u64) -> Result<u64, WorldError> {
    a.checked_add(b)
        .ok_or_else(|| invalid("aggregate", "overflow"))
}
fn restore_station_cargo(
    tables: &Tables,
    state: &mut DerivedState,
    cargo_seen: &mut BTreeSet<u32>,
) -> Result<(), WorldError> {
    for (id, row) in rows(tables, *b"STNN")? {
        if row.unsigned("facilities")? & 0x40 != 0 {
            continue;
        }
        let station = row.single("normal")?;
        for (cargo_type, goods) in station.child("goods")?.into_iter().enumerate() {
            let cargo_type = u32::try_from(cargo_type)
                .map_err(|_| invalid("STNN/goods", "cargo type overflow"))?;
            let mut destinations = BTreeSet::new();
            for group in goods.child("cargo")? {
                let next_hop = u32::try_from(group.unsigned("first")?)
                    .map_err(|_| invalid("STNN/goods", "destination overflow"))?;
                if !destinations.insert(next_hop) {
                    return Err(invalid("STNN/goods/cargo", "duplicate destination group"));
                }
                state.cargo_lists.push(super::cargo::aggregate(
                    tables,
                    Owner {
                        pool: Pool::Station,
                        id,
                    },
                    Some(cargo_type),
                    Some(next_hop),
                    group.references("second")?,
                    cargo_seen,
                )?);
            }
        }
    }
    Ok(())
}
