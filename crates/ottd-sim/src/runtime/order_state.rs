//! One authoritative order backing with native live allocation and save context.
mod airport;
mod backup;
use backup::BackupAllocation;
pub use backup::BackupReset;
#[cfg(test)]
mod backup_enabled_tests;
#[cfg(test)]
mod backup_sale_tests;
mod depot;
mod detach;
#[cfg(test)]
mod native;
mod observation;
pub(super) mod restore;
#[cfg(test)]
mod restore_boundary_tests;
#[cfg(test)]
mod restore_tests;
mod sale;
#[cfg(test)]
mod sale_tests;
#[cfg(test)]
mod shared_restore_tests;
#[cfg(test)]
mod tests;
mod view;
use super::{
    RuntimeError, SimulationRuntime, VehicleId,
    pools::{PoolAllocator, PoolSnapshot},
};
use ottd_save::{
    Savegame,
    world::{World, WorldEdit},
};
use serde::Serialize;
pub(super) use view::OrderReader;

/// Host role fixed at the runtime load boundary, never inferred from saved fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RuntimeSaveContext {
    /// Ordinary offline game; live backups are transient.
    SinglePlayer,
    /// Joining client retains server backups on load, but does not save them.
    NetworkClient,
    /// Server saves live backups but discards previously loaded backups.
    NetworkServer,
}
/// Explicit native afterload transition, including preserved pool allocation history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OrderLoadReceipt {
    /// Actual host role for this runtime lifetime.
    pub context: RuntimeSaveContext,
    /// Allocation state after decoding explicit backup IDs.
    pub before: PoolSnapshot,
    /// Backup IDs deleted individually by native afterload semantics.
    pub deleted: Vec<u32>,
    /// Allocation state after native afterload.
    pub after: PoolSnapshot,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OrderState {
    lists: PoolAllocator,
    backups: BackupAllocation,
    context: RuntimeSaveContext,
}
impl OrderState {
    pub(super) fn restore(
        world: &World,
        context: RuntimeSaveContext,
    ) -> Result<Self, RuntimeError> {
        Ok(Self {
            lists: PoolAllocator::restore(
                64000,
                128,
                view::table(world, *b"ORDL")?.records().keys().copied(),
            )?,
            backups: BackupAllocation::restore(
                view::table(world, *b"BKOR")?.records().keys().copied(),
            )?,
            context,
        })
    }
    fn publish(
        &mut self,
        world: &mut World,
        edits: Vec<WorldEdit>,
        backups: BackupAllocation,
    ) -> Result<(), RuntimeError> {
        let mut tx = world.transaction();
        for edit in edits {
            tx.apply(edit)?;
        }
        let prepared = tx.prepare()?;
        let ids: Vec<_> = OrderReader::Candidate(prepared.view())
            .rows(*b"BKOR")?
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        backups.validate_slots(&ids)?;
        prepared.commit();
        self.backups = backups;
        Ok(())
    }
}
impl SimulationRuntime {
    /// Restore a loaded v362 world, applying native role-dependent backup afterload.
    /// # Errors
    /// Rejects unsupported runtime input or invalid backup allocation/reference state.
    pub fn from_loaded_vanilla(
        mut world: World,
        context: RuntimeSaveContext,
    ) -> Result<(Self, OrderLoadReceipt), RuntimeError> {
        let (orders, receipt) = OrderState::from_loaded(&mut world, context)?;
        let runtime = Self::restore_with_orders(world, orders)?;
        Ok((runtime, receipt))
    }
    /// Observe native live backup allocation, including holes and retained capacity.
    pub fn order_backup_pool(&self) -> PoolSnapshot {
        self.orders.backups.snapshot()
    }
    /// Observe native order-list allocation without duplicating order records.
    pub fn order_list_pool(&self) -> PoolSnapshot {
        self.orders.lists.snapshot()
    }
    /// Immutable host role selected at load.
    pub const fn save_context(&self) -> RuntimeSaveContext {
        self.orders.context
    }
    /// Export the native host-role projection, leaving live state unchanged.
    /// # Errors
    /// Propagates save serialization failures.
    pub fn to_savegame(&self) -> Result<Savegame, ottd_save::world::WorldError> {
        match self.orders.context {
            RuntimeSaveContext::NetworkServer => self.world.to_savegame(),
            RuntimeSaveContext::SinglePlayer | RuntimeSaveContext::NetworkClient => {
                self.world.without_order_backups().to_savegame()
            }
        }
    }
    /// Observe the same native saved projection used by `to_savegame`.
    /// # Errors
    /// Propagates malformed saved-state export errors.
    pub fn saved_json(&self) -> Result<serde_json::Value, ottd_save::world::WorldError> {
        match self.orders.context {
            RuntimeSaveContext::NetworkServer => self.world.saved_json(),
            RuntimeSaveContext::SinglePlayer | RuntimeSaveContext::NetworkClient => {
                self.world.without_order_backups().saved_json()
            }
        }
    }
    /// Create/replace a live backup through native backup semantics.
    /// # Errors
    /// Rejects invalid vehicle/order references; never partially publishes.
    pub fn backup_orders(&mut self, vehicle: VehicleId, user: u32) -> Result<(), RuntimeError> {
        let (edits, pool) = backup::create(&self.orders, &self.world, vehicle, user)?;
        self.orders.publish(&mut self.world, edits, pool)
    }
}

impl SimulationRuntime {
    /// Apply the depot order/backup lifecycle primitive without deleting map objects.
    /// # Errors
    /// Rejects missing depot identity or unsupported airport backup classification.
    pub fn invalidate_depot_orders(&mut self, depot: u16, tile: u32) -> Result<(), RuntimeError> {
        let t = self
            .world
            .map()
            .tiles()
            .get(usize::try_from(tile).map_err(|_| RuntimeError::Invalid("depot tile"))?)
            .ok_or(RuntimeError::Invalid("depot tile"))?;
        if t.tile_type() >> 4 != 2 || t.m5() >> 6 != 2 || t.m2() != depot {
            return Err(RuntimeError::Invalid("depot order identity"));
        }
        let derived = &self.world.derived().order_lists;
        let plan = self.orders.plan_depot_invalidation(
            OrderReader::Committed(&self.world),
            derived,
            depot,
            tile,
            self.world.map().width(),
            self.world.map().height(),
        )?;
        let mut tx = self.world.transaction();
        let pending = plan.stage(&mut tx)?;
        let (prepared, delta) = pending.validate(tx.prepare()?)?;
        prepared.commit();
        delta.publish(&mut self.orders);
        Ok(())
    }
}

impl SimulationRuntime {
    /// Reset native live backups without GUI/network command dispatch.
    /// # Errors
    /// Rejects malformed backup data without partial deletion.
    pub fn reset_order_backups(&mut self, reset: BackupReset) -> Result<(), RuntimeError> {
        let (edits, pool) = backup::reset(&self.orders, &self.world, reset)?;
        self.orders.publish(&mut self.world, edits, pool)
    }
}
impl SimulationRuntime {
    /// Clear a deleted group's backup references without removing backups.
    /// # Errors
    /// Rejects malformed saved backup state before mutation.
    pub fn clear_order_backup_group(&mut self, group: u16) -> Result<(), RuntimeError> {
        let (edits, pool) = backup::clear_group(&self.orders, &self.world, group)?;
        self.orders.publish(&mut self.world, edits, pool)
    }
    /// Replace/delete backup clone references before a vehicle leaves its shared chain.
    /// This primitive does not admit ordered vehicle sale or unlink that vehicle.
    /// # Errors
    /// Rejects unsupported vehicle families or inconsistent shared metadata.
    pub fn clear_order_backup_vehicle(&mut self, vehicle: VehicleId) -> Result<(), RuntimeError> {
        let (edits, pool) = backup::clear_vehicle(&self.orders, &self.world, vehicle)?;
        self.orders.publish(&mut self.world, edits, pool)
    }
}

impl SimulationRuntime {
    /// Observe complete initialized order state and live allocation independently of saving.
    /// # Errors
    /// Rejects malformed records or unsupported effect/disaster order observations.
    pub fn order_state_json(&self) -> Result<serde_json::Value, RuntimeError> {
        self.orders.observe(&self.world)
    }
}

impl OrderState {
    fn from_loaded(
        world: &mut World,
        context: RuntimeSaveContext,
    ) -> Result<(Self, OrderLoadReceipt), RuntimeError> {
        let mut orders = Self::restore(world, context)?;
        let before = orders.backups.snapshot();
        let deleted = match context {
            RuntimeSaveContext::NetworkClient => Vec::new(),
            RuntimeSaveContext::SinglePlayer | RuntimeSaveContext::NetworkServer => {
                before.occupied.clone()
            }
        };
        let mut pool = orders.backups.clone();
        let mut edits = Vec::new();
        for id in &deleted {
            pool.free(*id)?;
            edits.push(WorldEdit::RemoveRecord {
                chunk: *b"BKOR",
                record: *id,
            });
        }
        if !edits.is_empty() {
            orders.publish(world, edits, pool)?;
        }
        let receipt = OrderLoadReceipt {
            context,
            before,
            deleted,
            after: orders.backups.snapshot(),
        };
        Ok((orders, receipt))
    }
}

pub(super) fn purchase_tile(
    schema: &ottd_save::TableSchema,
    record: &ottd_save::TableRecord,
) -> Result<u32, RuntimeError> {
    let row = view::vehicle(view::Row { schema, record })?
        .ok_or(RuntimeError::Invalid("purchase vehicle family"))?
        .1;
    u32::try_from(row.number("tile")?).map_err(|_| RuntimeError::Invalid("purchase tile"))
}
