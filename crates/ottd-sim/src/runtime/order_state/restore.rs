mod properties;
use super::{
    BackupAllocation, OrderState, PoolAllocator, RuntimeError, RuntimeSaveContext, VehicleId, view,
};
use ottd_save::{
    TableRecord, WireValue,
    world::{OrderListState, PreparedWorldTransaction, World, WorldEdit, WorldTransaction},
};
use properties::field;

pub(in crate::runtime) struct RestorePlan {
    slot: Option<u32>,
    lists: PoolAllocator,
    backups: BackupAllocation,
    previous: Vec<OrderListState>,
}
pub(in crate::runtime) struct PendingRestore {
    plan: RestorePlan,
    created: Option<u32>,
    vehicle: VehicleId,
}
pub(in crate::runtime) struct PreparedRestore {
    lists: PoolAllocator,
    backups: BackupAllocation,
}
impl OrderState {
    pub(in crate::runtime) fn admit_restore(
        &self,
        world: &World,
        tile: u32,
        user: u32,
    ) -> Result<Option<u32>, RuntimeError> {
        let table = view::table(world, *b"BKOR")?;
        if !table.records().is_empty() && self.context != RuntimeSaveContext::SinglePlayer {
            return Err(RuntimeError::Unsupported(
                "purchase live backup host context",
            ));
        }
        let mut matched = None;
        for (id, record) in table.records() {
            let row = view::Row {
                schema: table.schema(),
                record,
            };
            if row.number("tile")? != u64::from(tile) || row.number("user")? != u64::from(user) {
                continue;
            }
            if row.number("clone")? != 0 {
                return Err(RuntimeError::Unsupported("purchase shared backup restore"));
            }
            if row.number("group")? != 65534 {
                return Err(RuntimeError::Unsupported(
                    "purchase nondefault backup group",
                ));
            }
            if matched.replace(*id).is_some() {
                return Err(RuntimeError::Invalid("duplicate live backup user"));
            }
        }
        Ok(matched)
    }
    pub(in crate::runtime) fn plan_restore(
        &self,
        world: &World,
        tile: u32,
        user: u32,
    ) -> Result<RestorePlan, RuntimeError> {
        let slot = self.admit_restore(world, tile, user)?;
        self.backups.validate_slots(
            &view::table(world, *b"BKOR")?
                .records()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
        )?;
        if self.lists.snapshot().occupied
            != view::table(world, *b"ORDL")?
                .records()
                .keys()
                .copied()
                .collect::<Vec<_>>()
        {
            return Err(RuntimeError::Invalid("restore list allocator membership"));
        }
        Ok(RestorePlan {
            slot,
            lists: self.lists.clone(),
            backups: self.backups.clone(),
            previous: world.derived().order_lists.clone(),
        })
    }
}
impl RestorePlan {
    pub(in crate::runtime) fn stage(
        mut self,
        tx: &mut WorldTransaction<'_>,
        vehicle: VehicleId,
    ) -> Result<PendingRestore, RuntimeError> {
        let mut created = None;
        if let Some(slot) = self.slot {
            let candidate = tx.view();
            let backups = candidate
                .table(*b"BKOR")
                .ok_or(RuntimeError::Invalid("BKOR"))?;
            let backup = view::Row {
                schema: backups.schema(),
                record: backups
                    .record(slot)
                    .ok_or(RuntimeError::Invalid("restore backup"))?,
            };
            let vehicles = candidate
                .table(*b"VEHS")
                .ok_or(RuntimeError::Invalid("VEHS"))?;
            let current = view::vehicle(view::Row {
                schema: vehicles.schema(),
                record: vehicles
                    .record(vehicle.raw())
                    .ok_or(RuntimeError::Invalid("restore vehicle"))?,
            })?
            .ok_or(RuntimeError::Invalid("restore vehicle family"))?
            .1;
            if current.number("orders")? != 0 || current.number("next_shared")? != 0 {
                return Err(RuntimeError::Invalid(
                    "restore requires new unshared vehicle",
                ));
            }
            let (schema, orders) = backup.children("orders")?;
            let mut edits = Vec::new();
            if !orders.is_empty() && self.lists.can_allocate(1) {
                let id = self.lists.allocate()?;
                let list_schema = candidate
                    .table(*b"ORDL")
                    .ok_or(RuntimeError::Invalid("ORDL"))?
                    .schema();
                if list_schema.fields().len() != 1
                    || list_schema
                        .fields()
                        .first()
                        .is_none_or(|field| field.name() != "orders")
                {
                    return Err(RuntimeError::Invalid("restore order list schema"));
                }
                edits.push(WorldEdit::InsertRecord {
                    chunk: *b"ORDL",
                    record: id,
                    value: TableRecord::new(vec![WireValue::Structs(orders.to_vec())]),
                });
                edits.push(field(
                    vehicle,
                    "orders",
                    WireValue::Unsigned(u64::from(id) + 1),
                ));
                created = Some(id);
            }
            edits.extend(properties::copy(
                backup,
                current,
                vehicles,
                vehicle,
                created.map(|_| (schema, orders)),
            )?);
            self.backups.free(slot)?;
            edits.push(WorldEdit::RemoveRecord {
                chunk: *b"BKOR",
                record: slot,
            });
            for edit in edits {
                tx.apply(edit)?;
            }
        }
        Ok(PendingRestore {
            plan: self,
            created,
            vehicle,
        })
    }
}
impl PendingRestore {
    pub(in crate::runtime) fn validate(
        self,
        prepared: &PreparedWorldTransaction<'_>,
    ) -> Result<PreparedRestore, RuntimeError> {
        let view = prepared.view();
        let ids = view
            .table(*b"BKOR")
            .ok_or(RuntimeError::Invalid("BKOR"))?
            .records()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        self.plan.backups.validate_slots(&ids)?;
        if self.plan.lists.snapshot().occupied
            != view
                .table(*b"ORDL")
                .ok_or(RuntimeError::Invalid("ORDL"))?
                .records()
                .map(|(id, _)| id)
                .collect::<Vec<_>>()
        {
            return Err(RuntimeError::Invalid("restore candidate list membership"));
        }
        let mut previous = Vec::new();
        for list in &prepared.derived().order_lists {
            if Some(list.id) == self.created {
                if list.first_shared != Some(self.vehicle.raw())
                    || list.vehicles != [self.vehicle.raw()]
                {
                    return Err(RuntimeError::Invalid("restore candidate list owner"));
                }
            } else {
                previous.push(list.clone());
            }
        }
        if previous != self.plan.previous {
            return Err(RuntimeError::Invalid(
                "restore changed resident order caches",
            ));
        }
        Ok(PreparedRestore {
            lists: self.plan.lists,
            backups: self.plan.backups,
        })
    }
}
impl PreparedRestore {
    pub(in crate::runtime) fn publish(self, state: &mut OrderState) {
        state.lists = self.lists;
        state.backups = self.backups;
    }
}
