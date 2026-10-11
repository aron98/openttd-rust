//! Candidate validation and atomic publication for runtime command contexts.
use super::{
    DepotContext, RoadVehicleContext, RuntimeError, VehicleAllocation, VehicleId, creation_cache,
    order_state,
};
use super::{pools, road_record};
use ottd_save::world::World;
#[derive(Debug, Clone, Copy)]
pub struct RoadBuildState {
    pub owner: u8,
    pub unit: u16,
    pub tile: u32,
    pub x: u32,
    pub y: u32,
    pub z: i32,
    pub direction: u8,
    pub engine: u16,
    pub image: u8,
    pub cargo: u8,
    pub capacity: u16,
    pub reliability: u16,
    pub reliability_decay: u16,
    pub max_age: i32,
    pub economy_date: i32,
    pub calendar_date: i32,
    pub build_year: i32,
    pub service_interval: u16,
    pub service_percent: bool,
    pub preview: bool,
    pub value: i64,
    pub random_bits: u16,
}
pub fn new_road_record(
    schema: &ottd_save::TableSchema,
    state: RoadBuildState,
    cargo_paid_for: u16,
) -> Result<ottd_save::TableRecord, RuntimeError> {
    road_record::build(schema, state, cargo_paid_for)
}

impl DepotContext<'_> {
    pub(crate) fn publish(
        self,
        world: &mut World,
        edits: Vec<ottd_save::world::WorldEdit>,
        pool: Option<pools::PoolAllocator>,
        infrastructure: Option<(u8, u8, u32)>,
    ) -> Result<(), crate::CommandError> {
        let counter = match infrastructure {
            Some((company, road_type, count)) => Some((
                self.road
                    .get_mut(&company)
                    .and_then(|v| v.get_mut(usize::from(road_type)))
                    .ok_or(RuntimeError::Invalid("depot infrastructure"))?,
                count,
            )),
            None => None,
        };
        let mut transaction = world.transaction();
        for edit in edits {
            transaction.apply(edit)?;
        }
        transaction.prepare()?.commit();
        if let Some(pool) = pool {
            *self.pool = pool;
        }
        if let Some((target, value)) = counter {
            *target = value;
        }
        Ok(())
    }
}
impl RoadVehicleContext<'_> {
    pub(crate) fn admit_restore(
        &self,
        world: &World,
        tile: u32,
        user: u32,
        company: u8,
        cargo: u8,
    ) -> Result<(), crate::CommandError> {
        if !world.tables().contains_key(b"BKOR") {
            return Err(crate::CommandError::Unsupported(
                "vehicle order-backup restore",
            ));
        }
        if let Some(slot) = self.orders.admit_restore(world, tile, user)? {
            order_state::restore::shared::admit(world, slot, company, cargo, self.content)?;
        }
        Ok(())
    }
    pub(crate) fn publish(
        self,
        world: &mut World,
        edits: Vec<ottd_save::world::WorldEdit>,
        allocation: VehicleAllocation,
        id: VehicleId,
        user: u32,
    ) -> Result<(), crate::CommandError> {
        let tile = edits
            .iter()
            .find_map(|edit| match edit {
                ottd_save::world::WorldEdit::InsertRecord {
                    chunk,
                    record,
                    value,
                } if *chunk == *b"VEHS" && *record == id.raw() => Some(value),
                _ => None,
            })
            .ok_or(RuntimeError::Invalid("purchase constructor record"))?;
        let schema = world
            .tables()
            .get(b"VEHS")
            .ok_or(RuntimeError::Invalid("VEHS"))?
            .schema();
        let tile = order_state::purchase_tile(schema, tile)?;
        let restore = self.orders.plan_restore(world, tile, user)?;
        let mut transaction = world.transaction();
        for edit in edits {
            transaction.apply(edit)?;
        }
        let pending = restore.stage(&mut transaction, id, self.content)?;
        let prepared = transaction.prepare()?;
        let restored = pending.validate(&prepared)?;
        if allocation.pool.snapshot().occupied
            != prepared
                .view()
                .table(*b"VEHS")
                .ok_or(RuntimeError::Invalid("VEHS"))?
                .records()
                .map(|(id, _)| id)
                .collect::<Vec<_>>()
        {
            return Err(RuntimeError::Invalid("purchase candidate vehicle membership").into());
        }
        let cache = creation_cache(prepared.view(), id, self.content)?;
        prepared.commit();
        restored.publish(self.orders);
        self.road.insert(id, cache);
        *self.allocation = allocation;
        Ok(())
    }
}
