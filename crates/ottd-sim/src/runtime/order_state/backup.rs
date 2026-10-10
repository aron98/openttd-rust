use super::{OrderState, PoolAllocator, RuntimeError, VehicleId, view};
use ottd_save::{
    TableRecord, WireValue,
    world::{World, WorldEdit},
};
use std::collections::BTreeMap;
pub(super) fn create(
    state: &OrderState,
    world: &World,
    id: VehicleId,
    user: u32,
) -> Result<(Vec<WorldEdit>, BackupAllocation), RuntimeError> {
    let vehicle = view::consist(world, id.raw())?;
    let mut edits = Vec::new();
    let mut pool = state.backups.clone();
    let backups = view::table(world, *b"BKOR")?;
    for (key, record) in backups.records() {
        if (view::Row {
            schema: backups.schema(),
            record,
        })
        .number("user")?
            == u64::from(user)
        {
            pool.free(*key)?;
            edits.push(WorldEdit::RemoveRecord {
                chunk: *b"BKOR",
                record: *key,
            });
        }
    }
    if !pool.can_allocate(1) {
        return Ok((edits, pool));
    }
    let list = vehicle.number("orders")?;
    let mut clone = 0;
    let mut owned = Vec::new();
    if list != 0 {
        let list_id = u32::try_from(list.saturating_sub(1))
            .map_err(|_| RuntimeError::Invalid("order list ID"))?;
        let shared = world
            .derived()
            .order_lists
            .iter()
            .find(|l| l.id == list_id)
            .ok_or(RuntimeError::Invalid("order list metadata"))?;
        if shared.vehicles.len() > 1 {
            clone = shared
                .vehicles
                .iter()
                .find(|v| **v != id.raw())
                .map(|v| u64::from(*v) + 1)
                .ok_or(RuntimeError::Invalid("shared backup clone"))?;
        } else {
            owned = view::row(world, *b"ORDL", list_id)?
                .children("orders")?
                .1
                .to_vec();
        }
    }
    let mut values = Vec::new();
    for field in backups.schema().fields() {
        let value = match field.name() {
            "user" => WireValue::Unsigned(u64::from(user)),
            "tile" => vehicle.field("tile")?.clone(),
            "group" => vehicle.field("group_id")?.clone(),
            "clone" => WireValue::Unsigned(clone),
            "orders" => WireValue::Structs(std::mem::take(&mut owned)),
            "vehicle_flags" => WireValue::Unsigned(vehicle.number("vehicle_flags")? & 0x338),
            "name" => vehicle.field("name")?.clone(),
            "service_interval" => vehicle.field("service_interval")?.clone(),
            "cur_real_order_index" => vehicle.field("cur_real_order_index")?.clone(),
            "cur_implicit_order_index" => vehicle.field("cur_implicit_order_index")?.clone(),
            "current_order_time" => WireValue::Unsigned(u64::from(u32::from_le_bytes(
                i32::try_from(vehicle.signed("current_order_time")?)
                    .map_err(|_| RuntimeError::Invalid("backup current order time"))?
                    .to_le_bytes(),
            ))),
            "lateness_counter" => vehicle.field("lateness_counter")?.clone(),
            "timetable_start" => vehicle.field("timetable_start")?.clone(),
            _ => return Err(RuntimeError::Invalid("backup field inventory")),
        };
        values.push(value);
    }
    edits.push(WorldEdit::InsertRecord {
        chunk: *b"BKOR",
        record: pool.allocate()?,
        value: TableRecord::new(values),
    });
    Ok((edits, pool))
}
/// Native game-logic backup reset predicate; this is not a GUI network command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupReset {
    /// Remove all users' backups at one tile.
    AtTile(u32),
    /// Remove every backup.
    All,
    /// Remove one user's backups, optionally restricting the tile.
    User {
        /// Native user/client identity.
        user: u32,
        /// None is native `INVALID_TILE` wildcard.
        tile: Option<u32>,
    },
}
pub(super) fn reset(
    state: &OrderState,
    world: &World,
    reset: BackupReset,
) -> Result<(Vec<WorldEdit>, BackupAllocation), RuntimeError> {
    let mut pool = state.backups.clone();
    let mut edits = Vec::new();
    let table = view::table(world, *b"BKOR")?;
    for (id, record) in table.records() {
        let row = view::Row {
            schema: table.schema(),
            record,
        };
        let row_tile = row.number("tile")?;
        let row_user = row.number("user")?;
        let remove = match reset {
            BackupReset::All => true,
            BackupReset::AtTile(tile) => tile == u32::MAX || row_tile == u64::from(tile),
            BackupReset::User { user, tile } => {
                row_user == u64::from(user)
                    && tile.is_none_or(|tile| tile == u32::MAX || row_tile == u64::from(tile))
            }
        };
        if remove {
            pool.free(*id)?;
            edits.push(WorldEdit::RemoveRecord {
                chunk: *b"BKOR",
                record: *id,
            });
        }
    }
    Ok((edits, pool))
}
pub(super) fn clear_group(
    state: &OrderState,
    world: &World,
    group: u16,
) -> Result<(Vec<WorldEdit>, BackupAllocation), RuntimeError> {
    let table = view::table(world, *b"BKOR")?;
    let mut edits = Vec::new();
    for (id, record) in table.records() {
        if (view::Row {
            schema: table.schema(),
            record,
        })
        .number("group")?
            == u64::from(group)
        {
            edits.push(WorldEdit::Field {
                chunk: *b"BKOR",
                record: *id,
                path: vec![ottd_save::world::PathElement::Field("group".into())],
                value: WireValue::Unsigned(65534),
            });
        }
    }
    Ok((edits, state.backups.clone()))
}
pub(super) fn clear_vehicle(
    state: &OrderState,
    world: &World,
    id: VehicleId,
) -> Result<(Vec<WorldEdit>, BackupAllocation), RuntimeError> {
    let source = view::consist(world, id.raw())?;
    let list = source.number("orders")?;
    let next = if list == 0 {
        None
    } else {
        let list_id = u32::try_from(list.saturating_sub(1))
            .map_err(|_| RuntimeError::Invalid("backup clone list"))?;
        world
            .derived()
            .order_lists
            .iter()
            .find(|v| v.id == list_id)
            .ok_or(RuntimeError::Invalid("backup clone membership"))?
            .vehicles
            .iter()
            .find(|v| **v != id.raw())
            .copied()
    };
    let mut pool = state.backups.clone();
    let mut edits = Vec::new();
    let table = view::table(world, *b"BKOR")?;
    for (key, record) in table.records() {
        if (view::Row {
            schema: table.schema(),
            record,
        })
        .number("clone")?
            != u64::from(id.raw()) + 1
        {
            continue;
        }
        let edit = if let Some(next) = next {
            WorldEdit::Field {
                chunk: *b"BKOR",
                record: *key,
                path: vec![ottd_save::world::PathElement::Field("clone".into())],
                value: WireValue::Unsigned(u64::from(next) + 1),
            }
        } else {
            pool.free(*key)?;
            WorldEdit::RemoveRecord {
                chunk: *b"BKOR",
                record: *key,
            }
        };
        edits.push(edit);
    }
    Ok((edits, pool))
}

/// Physical pool identity and the distinct native object's initialized index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BackupAllocation {
    pool: PoolAllocator,
    object_indices: BTreeMap<u32, u8>,
}
impl BackupAllocation {
    pub(super) fn restore(ids: impl Iterator<Item = u32>) -> Result<Self, RuntimeError> {
        let ids: Vec<_> = ids.collect();
        let pool = PoolAllocator::restore(255, 1, ids.iter().copied())?;
        // BKOR Load value-initializes the defaulted constructor after placement allocation.
        let object_indices = ids.into_iter().map(|id| (id, 0)).collect();
        Ok(Self {
            pool,
            object_indices,
        })
    }
    pub(super) fn snapshot(&self) -> super::PoolSnapshot {
        self.pool.snapshot()
    }
    pub(super) fn can_allocate(&self, count: u32) -> bool {
        self.pool.can_allocate(count)
    }
    pub(super) fn allocate(&mut self) -> Result<u32, RuntimeError> {
        let id = self.pool.allocate()?;
        let index =
            u8::try_from(id).map_err(|_| RuntimeError::Invalid("backup allocated index"))?;
        if self.object_indices.insert(id, index).is_some() {
            return Err(RuntimeError::Invalid("duplicate backup object identity"));
        }
        Ok(id)
    }
    pub(super) fn object_index(&self, slot: u32) -> Result<u8, RuntimeError> {
        self.object_indices
            .get(&slot)
            .copied()
            .ok_or(RuntimeError::Invalid("missing backup object index"))
    }
    pub(super) fn free(&mut self, slot: u32) -> Result<(), RuntimeError> {
        let index = self.object_index(slot)?;
        if u32::from(index) != slot {
            return Err(RuntimeError::NativeBackupIndex { slot, index });
        }
        self.pool.free(slot)?;
        if self.object_indices.remove(&slot).is_none() {
            return Err(RuntimeError::Invalid("missing freed backup metadata"));
        }
        Ok(())
    }
    pub(super) fn validate_slots(&self, ids: &[u32]) -> Result<(), RuntimeError> {
        if self.snapshot().occupied != ids
            || !self.object_indices.keys().copied().eq(ids.iter().copied())
        {
            return Err(RuntimeError::Invalid(
                "backup slot/object metadata membership",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CommandReturn,
        runtime::purchase_tests::fixture::{fixture, request},
    };
    #[test]
    fn candidate_metadata_membership_must_match_all_live_backup_slots()
    -> Result<(), Box<dyn std::error::Error>> {
        // Given one live production-created backup and unchanged authoritative state.
        let (mut runtime, tile) = fixture()?;
        let Some(CommandReturn::Vehicle { vehicle, .. }) = runtime
            .execute_command(&request(tile))?
            .returns
            .ok_or("returns")?
            .result
        else {
            return Err("vehicle".into());
        };
        runtime.backup_orders(VehicleId::new(vehicle), 42)?;
        let before = runtime.world.saved_json()?;
        let pool = runtime.order_backup_pool();
        let mut candidate = runtime.orders.backups.clone();
        candidate.object_indices.clear();
        // When the prepared candidate lacks its native object-index metadata.
        let result = runtime
            .orders
            .publish(&mut runtime.world, Vec::new(), candidate);
        // Then preparation cannot publish even an otherwise empty edit set.
        assert!(result.is_err());
        assert_eq!(runtime.world.saved_json()?, before);
        assert_eq!(runtime.order_backup_pool(), pool);
        Ok(())
    }
}
