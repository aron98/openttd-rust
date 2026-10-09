use super::derived::{
    CargoPayment, DerivedState, GroupChildren, Owner, Pool, RoadStopChain, StorageOwner,
};
use super::{TableChunk, WorldError, invalid, rows};
use std::collections::{BTreeMap, BTreeSet};
type Tables = BTreeMap<[u8; 4], TableChunk>;
pub(super) fn restore_groups(tables: &Tables, state: &mut DerivedState) -> Result<(), WorldError> {
    let groups: BTreeMap<_, _> = rows(tables, *b"GRPS")?.collect();
    let mut parents = BTreeMap::new();
    for (id, row) in &groups {
        let raw = u32::try_from(row.unsigned("parent")?)
            .map_err(|_| invalid("GRPS", "parent overflow"))?;
        let parent = groups
            .get(&raw)
            .filter(|p| {
                p.unsigned("owner").ok() == row.unsigned("owner").ok()
                    && p.unsigned("vehicle_type").ok() == row.unsigned("vehicle_type").ok()
            })
            .map(|_| raw);
        parents.insert(*id, parent);
    }
    // Unlike singly linked chains, groups may have multiple children.
    let mut finished = BTreeSet::new();
    for id in groups.keys() {
        let mut branch = BTreeSet::new();
        let mut current = Some(*id);
        while let Some(id) = current {
            if finished.contains(&id) {
                break;
            }
            if !branch.insert(id) {
                return Err(invalid("GRPS", "cyclic group hierarchy"));
            }
            current = parents.get(&id).copied().flatten();
        }
        finished.extend(branch);
    }
    for id in groups.keys() {
        state.groups.push(GroupChildren {
            id: *id,
            children: parents
                .iter()
                .filter_map(|(child, parent)| (*parent == Some(*id)).then_some(*child))
                .collect(),
        });
    }
    Ok(())
}
pub(super) fn restore_ownership(
    tables: &Tables,
    state: &mut DerivedState,
) -> Result<(), WorldError> {
    let stops: BTreeMap<_, _> = rows(tables, *b"ROAD")?
        .map(|(id, row)| Ok((id, row.reference("next")?)))
        .collect::<Result<_, WorldError>>()?;
    let mut used = BTreeSet::new();
    let mut storage = BTreeSet::new();
    for (id, row) in rows(tables, *b"STNN")? {
        if row.unsigned("facilities")? & 0x40 != 0 {
            continue;
        }
        let station = row.single("normal")?;
        for (field, kind) in [("bus_stops", "bus"), ("truck_stops", "truck")] {
            let mut current = station.reference(field)?;
            let mut chain = Vec::new();
            while let Some(stop) = current {
                if !used.insert(stop) {
                    return Err(invalid("ROAD", "roadstop has multiple owners"));
                }
                chain.push(stop);
                current = stops.get(&stop).copied().flatten();
            }
            state.road_stop_chains.push(RoadStopChain {
                station: id,
                kind,
                stops: chain,
            });
        }
        if let Some(psa) = station.reference("airport.psa")? {
            storage_owner(
                state,
                &mut storage,
                psa,
                Owner {
                    pool: Pool::Station,
                    id,
                },
            )?;
        }
    }
    for (id, row) in rows(tables, *b"INDY")? {
        if let Some(psa) = row.reference("psa")? {
            storage_owner(
                state,
                &mut storage,
                psa,
                Owner {
                    pool: Pool::Industry,
                    id,
                },
            )?;
        }
    }
    for (id, row) in rows(tables, *b"CITY")? {
        for psa in row.references("psa_list")? {
            storage_owner(
                state,
                &mut storage,
                psa,
                Owner {
                    pool: Pool::Town,
                    id,
                },
            )?;
        }
    }
    state.storage_owners.sort_by_key(|owner| owner.id);
    let mut payments = BTreeSet::new();
    for (id, row) in rows(tables, *b"CAPY")? {
        let vehicle = row
            .reference("front")?
            .ok_or_else(|| invalid("CAPY/front", "null front vehicle"))?;
        if !payments.insert(vehicle) {
            return Err(invalid("CAPY", "vehicle has multiple cargo payments"));
        }
        state.cargo_payments.push(CargoPayment { id, vehicle });
    }
    Ok(())
}
fn storage_owner(
    state: &mut DerivedState,
    seen: &mut BTreeSet<u32>,
    id: u32,
    owner: Owner,
) -> Result<(), WorldError> {
    if !seen.insert(id) {
        return Err(invalid("PSAC", "storage has multiple owners"));
    }
    state.storage_owners.push(StorageOwner { id, owner });
    Ok(())
}
