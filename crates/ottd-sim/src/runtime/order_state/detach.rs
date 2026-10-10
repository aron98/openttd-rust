use super::{OrderState, PoolAllocator, RuntimeError, VehicleId};
use crate::runtime::SavedVehicleView;
use PathElement::{Field, Index};
use ottd_save::{
    WireValue,
    world::{
        OrderListState, PathElement, PreparedWorldTransaction, World, WorldEdit, WorldTransaction,
    },
};

#[derive(Debug)]
pub(in crate::runtime) struct DetachPlan {
    edits: Vec<WorldEdit>,
    pending: PendingDetach,
}
#[derive(Debug)]
pub(in crate::runtime) struct PendingDetach {
    expected: Vec<OrderListState>,
    lists: PoolAllocator,
}
#[derive(Debug)]
pub(in crate::runtime) struct PreparedDetach {
    lists: PoolAllocator,
}
impl OrderState {
    pub(in crate::runtime) fn plan_detach(
        &self,
        world: &World,
        id: VehicleId,
    ) -> Result<DetachPlan, RuntimeError> {
        let table = world
            .tables()
            .get(b"ORDL")
            .ok_or(RuntimeError::Invalid("ORDL"))?;
        if self.lists.snapshot().occupied != table.records().keys().copied().collect::<Vec<_>>() {
            return Err(RuntimeError::Invalid("order list allocator membership"));
        }
        let mut lists = self.lists.clone();
        let mut expected = world.derived().order_lists.clone();
        let mut edits = Vec::new();
        let reference = SavedVehicleView::new(world, id)?.common_number("orders")?;
        if let Some(raw) = reference.checked_sub(1) {
            let list_id =
                u32::try_from(raw).map_err(|_| RuntimeError::Invalid("order reference"))?;
            let index = expected
                .iter()
                .position(|list| list.id == list_id)
                .ok_or(RuntimeError::Invalid("order list cache"))?;
            let list = expected
                .get_mut(index)
                .ok_or(RuntimeError::Invalid("order list cache"))?;
            let member = list
                .vehicles
                .iter()
                .position(|vehicle| *vehicle == id.raw())
                .ok_or(RuntimeError::Invalid("order list member"))?;
            if let Some(previous) = member.checked_sub(1).and_then(|at| list.vehicles.get(at)) {
                let next = list
                    .vehicles
                    .get(member.saturating_add(1))
                    .map_or(0, |next| u64::from(*next) + 1);
                edits.push(WorldEdit::Field {
                    chunk: *b"VEHS",
                    record: *previous,
                    path: vec![
                        Field("roadveh".into()),
                        Index(0),
                        Field("common".into()),
                        Index(0),
                        Field("next_shared".into()),
                    ],
                    value: WireValue::Unsigned(next),
                });
            }
            list.vehicles.remove(member);
            list.first_shared = list.vehicles.first().copied();
            if list.vehicles.is_empty() {
                edits.push(WorldEdit::RemoveRecord {
                    chunk: *b"ORDL",
                    record: list_id,
                });
                lists.free(list_id)?;
                expected.remove(index);
            }
        }
        Ok(DetachPlan {
            edits,
            pending: PendingDetach { expected, lists },
        })
    }
}
impl DetachPlan {
    pub(in crate::runtime) fn stage(
        self,
        tx: &mut WorldTransaction<'_>,
    ) -> Result<PendingDetach, RuntimeError> {
        for edit in self.edits {
            tx.apply(edit)?;
        }
        Ok(self.pending)
    }
}
impl PendingDetach {
    pub(in crate::runtime) fn validate(
        self,
        prepared: &PreparedWorldTransaction<'_>,
    ) -> Result<PreparedDetach, RuntimeError> {
        let ids = prepared
            .view()
            .table(*b"ORDL")
            .ok_or(RuntimeError::Invalid("ORDL"))?
            .records()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        if self.lists.snapshot().occupied != ids || self.expected != prepared.derived().order_lists
        {
            return Err(RuntimeError::Invalid("sale order detach candidate"));
        }
        Ok(PreparedDetach { lists: self.lists })
    }
}
impl PreparedDetach {
    pub(in crate::runtime) fn publish(self, state: &mut OrderState) {
        state.lists = self.lists;
    }
}
