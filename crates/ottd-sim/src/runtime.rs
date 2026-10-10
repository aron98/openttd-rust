//! Vanilla runtime over one authoritative saved world and derived road caches.
mod allocation;
mod order_state;
pub use order_state::{BackupReset, OrderLoadReceipt, RuntimeSaveContext};
#[cfg(test)]
mod backup_enabled_sale_native;
#[cfg(test)]
mod backup_sale_native;
mod depot;
mod depot_removal;
#[cfg(test)]
mod depot_removal_native;
mod group_counts;
#[cfg(test)]
mod ordered_sale_native;
#[cfg(test)]
mod owned_restore_native;
pub mod pools;
#[cfg(test)]
mod purchase_native;
#[cfg(test)]
mod purchase_tests;
mod road_cache;
mod road_record;
mod road_sale;
pub use group_counts::{CompanyRoadCounts, RoadGroupCounts};
#[cfg(test)]
mod sale_tests;
mod saved_engine;
mod saved_vehicle;
mod serialization;
use crate::content::{ContentCatalog, ContentError};
use ottd_save::world::World;
pub use saved_engine::SavedEngineView;
pub use saved_vehicle::SavedVehicleView;
use serde::Serialize;
use std::collections::BTreeMap;

/// Native sparse vehicle pool identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct VehicleId(u32);
impl VehicleId {
    /// Wrap a pool index; lookups check whether it exists.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    /// Native pool index, not a saved pointer encoding.
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Rejected or malformed runtime restoration input.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// Invalid native vehicle allocation metadata.
    #[error(transparent)]
    Pool(#[from] pools::PoolError),
    /// Saved order backups require an explicit host load context.
    #[error("loaded order backups require explicit runtime context")]
    OrderContextRequired,
    /// Pinned native deletion would assert because a loaded object lost its pool index.
    #[error("native backup deletion invariant: physical slot {slot} has object index {index}")]
    NativeBackupIndex {
        /// Authoritative physical pool slot / saved row ID.
        slot: u32,
        /// Original runtime object's index after constructor/load.
        index: u8,
    },
    /// Invalid saved transaction.
    #[error(transparent)]
    World(#[from] ottd_save::world::WorldError),
    /// Unsupported content configuration.
    #[error(transparent)]
    Content(#[from] ContentError),
    /// Feature requiring a later runtime family.
    #[error("unsupported runtime: {0}")]
    Unsupported(&'static str),
    /// Missing or invalid native saved field.
    #[error("invalid runtime input: {0}")]
    Invalid(&'static str),
}
/// Native road caches after final after-load initialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoadVehicleCache {
    /// Native vehicle pool ID.
    pub id: VehicleId,
    /// Resolved native road type.
    pub road_type: u8,
    /// Native powered road-type mask.
    pub compatible_roadtypes: u64,
    /// Front engine ID for trailing parts; none for a front.
    pub first_engine: Option<u16>,
    /// Length in eighths of a standard vehicle.
    pub vehicle_length: u8,
    /// Consist length in the same units.
    pub total_length: u16,
    /// Native internal maximum speed.
    pub max_speed: u16,
    /// Ticks between cargo aging.
    pub cargo_age_period: u16,
    /// Resolved visual-effect bit field.
    pub visual_effect: u8,
    /// Consist weight in tonnes, clamped to at least one.
    pub weight: u32,
    /// Part slope resistance in newtons.
    pub slope_resistance: u32,
    /// Native narrowed axle resistance.
    pub axle_resistance: u16,
    /// Consist power in horsepower.
    pub power: u32,
    /// Native maximum tractive effort.
    pub max_tractive_effort: u32,
    /// Road-type-limited internal speed.
    pub max_track_speed: u16,
    /// Native air-drag coefficient.
    pub air_drag: u32,
    /// Load-initialized display-speed cache.
    pub last_speed: u16,
    /// Native load-initialized occupancy percentage.
    pub trip_occupancy: i8,
}

/// One authoritative saved world with immutable content and derived road caches.
#[derive(Debug)]
pub struct SimulationRuntime {
    world: World,
    orders: order_state::OrderState,
    content: ContentCatalog,
    road: BTreeMap<VehicleId, RoadVehicleCache>,
    allocation: VehicleAllocation,
    depot: depot::DepotRuntime,
    serializer_cargo_paid_for: u16,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VehicleAllocation {
    pub pool: pools::PoolAllocator,
    pub road_units: BTreeMap<u8, pools::UnitNumberAllocator>,
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct RoadBuildState {
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
pub(crate) fn new_road_record(
    schema: &ottd_save::TableSchema,
    state: RoadBuildState,
    cargo_paid_for: u16,
) -> Result<ottd_save::TableRecord, RuntimeError> {
    road_record::build(schema, state, cargo_paid_for)
}
pub(crate) struct DepotContext<'a> {
    orders: &'a mut order_state::OrderState,
    pub content: &'a ContentCatalog,
    pub pool: &'a mut pools::PoolAllocator,
    pub road: &'a mut BTreeMap<u8, [u32; 63]>,
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
pub(crate) struct RoadVehicleContext<'a> {
    orders: &'a mut order_state::OrderState,
    pub serializer_cargo_paid_for: u16,
    pub content: &'a ContentCatalog,
    pub allocation: &'a mut VehicleAllocation,
    pub road: &'a mut BTreeMap<VehicleId, RoadVehicleCache>,
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
pub(crate) fn creation_cache(
    view: ottd_save::world::CandidateView<'_>,
    id: VehicleId,
    content: &ContentCatalog,
) -> Result<RoadVehicleCache, RuntimeError> {
    road_cache::create(SavedVehicleView::candidate(view, id)?, content)
}
pub(crate) fn road_company_count(world: &World, company: u8) -> Result<u64, RuntimeError> {
    world
        .tables()
        .get(b"VEHS")
        .ok_or(RuntimeError::Invalid("VEHS"))?
        .records()
        .keys()
        .try_fold(0_u64, |count, id| {
            Ok(count.saturating_add(u64::from(
                SavedVehicleView::new(world, VehicleId::new(*id))?.owner()? == company,
            )))
        })
}
impl SimulationRuntime {
    /// Execute admitted service and road-purchase commands over the owned world.
    /// # Errors
    /// Rejects unsupported contexts and commands without live cache publication.
    pub fn execute_command(
        &mut self,
        request: &crate::CommandRequest,
    ) -> Result<crate::CommandReceipt, crate::CommandError> {
        if matches!(request.command, crate::Command::LandscapeClear { .. })
            && self.save_context() != RuntimeSaveContext::SinglePlayer
        {
            return Err(crate::CommandError::Unsupported(
                "depot removal host context",
            ));
        }
        match request.command {
            crate::Command::BuildRoadDepot { .. } | crate::Command::LandscapeClear { .. } => {
                DepotContext {
                    orders: &mut self.orders,
                    content: &self.content,
                    pool: &mut self.depot.pool,
                    road: &mut self.depot.road,
                }
                .execute(&mut self.world, request)
            }
            crate::Command::BuildVehicle { .. } | crate::Command::SellVehicle { .. } => {
                RoadVehicleContext {
                    orders: &mut self.orders,
                    serializer_cargo_paid_for: self.serializer_cargo_paid_for,
                    content: &self.content,
                    allocation: &mut self.allocation,
                    road: &mut self.road,
                }
                .execute(&mut self.world, request)
            }
            crate::Command::ChangeServiceInterval { .. } => {
                crate::commands::execute_command(&mut self.world, request)
            }
            crate::Command::LevelLand { .. }
            | crate::Command::TerraformLand { .. }
            | crate::Command::BuildRoad { .. }
            | crate::Command::IncreaseLoan { .. }
            | crate::Command::DecreaseLoan { .. }
            | crate::Command::RenameCompany { .. }
            | crate::Command::RenamePresident { .. }
            | crate::Command::Pause { .. } => Err(crate::CommandError::Unsupported(
                "runtime command cache publication",
            )),
        }
    }
    /// Restore vanilla single-part road vehicles without modifying saved fields or RNG.
    ///
    /// # Errors
    /// Rejects other vehicle families, articulated vehicles, invalid road tiles and mods.
    pub fn restore_vanilla(world: World) -> Result<Self, RuntimeError> {
        if world
            .tables()
            .get(b"BKOR")
            .is_none_or(|t| !t.records().is_empty())
        {
            return Err(RuntimeError::OrderContextRequired);
        }
        let orders = order_state::OrderState::restore(&world, RuntimeSaveContext::SinglePlayer)?;
        Self::restore_with_orders(world, orders)
    }
    fn restore_with_orders(
        world: World,
        orders: order_state::OrderState,
    ) -> Result<Self, RuntimeError> {
        let content = ContentCatalog::from_world(&world)?;
        let table = world
            .tables()
            .get(b"VEHS")
            .ok_or(RuntimeError::Invalid("VEHS"))?;
        let road = table
            .records()
            .keys()
            .map(|id| {
                let id = VehicleId::new(*id);
                let vehicle = SavedVehicleView::new(&world, id)?;
                let cache = road_cache::restore(vehicle, &content)?;
                Ok((id, cache))
            })
            .collect::<Result<BTreeMap<_, _>, RuntimeError>>()?;
        Ok(Self {
            orders,
            depot: depot::DepotRuntime::restore(&world)?,
            serializer_cargo_paid_for: serialization::restore(&world)?,
            allocation: VehicleAllocation::restore(&world)?,
            world,
            content,
            road,
        })
    }
    /// Canonical live records; runtime save methods apply transient backup projection.
    /// Mutable access is intentionally absent.
    pub const fn world(&self) -> &World {
        &self.world
    }

    /// Native depot pool identity metadata restored from the authoritative world.
    pub fn depot_pool(&self) -> pools::PoolSnapshot {
        self.depot.pool.snapshot()
    }

    /// Company road/tram infrastructure for all 63 native road-type slots.
    pub const fn road_infrastructure(&self) -> &BTreeMap<u8, [u32; 63]> {
        &self.depot.road
    }
    /// Initialized vanilla content snapshot.
    pub const fn content(&self) -> &ContentCatalog {
        &self.content
    }
    /// Restored road caches in native vehicle-ID order.
    pub const fn road_caches(&self) -> &BTreeMap<VehicleId, RoadVehicleCache> {
        &self.road
    }
    /// Lookup a restored road cache.
    /// # Errors
    /// Rejects absent vehicle IDs.
    pub fn road_cache(&self, id: VehicleId) -> Result<&RoadVehicleCache, RuntimeError> {
        self.road
            .get(&id)
            .ok_or(RuntimeError::Invalid("vehicle ID"))
    }
    /// Borrow saved road-vehicle fields without creating another mutable vehicle model.
    /// # Errors
    /// Rejects an absent or unsupported vehicle.
    pub fn vehicle(&self, id: VehicleId) -> Result<SavedVehicleView<'_>, RuntimeError> {
        SavedVehicleView::new(&self.world, id)
    }
    /// Borrow saved dynamic engine state.
    /// # Errors
    /// Rejects an absent engine.
    pub fn engine(&self, id: u16) -> Result<SavedEngineView<'_>, RuntimeError> {
        SavedEngineView::new(&self.world, id)
    }
    /// Release canonical live records, including transient backups.
    /// This is not a native save or runtime continuation: allocator history is not retained.
    pub fn into_world(self) -> World {
        self.world
    }
}
