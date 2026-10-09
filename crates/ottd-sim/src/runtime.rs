//! Read-only vanilla runtime restoration over one authoritative saved world.
pub mod pools;
mod road_cache;
mod saved_engine;
mod saved_vehicle;
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
    content: ContentCatalog,
    road: BTreeMap<VehicleId, RoadVehicleCache>,
}
impl SimulationRuntime {
    /// Restore vanilla single-part road vehicles without modifying saved fields or RNG.
    ///
    /// # Errors
    /// Rejects other vehicle families, articulated vehicles, invalid road tiles and mods.
    pub fn restore_vanilla(world: World) -> Result<Self, RuntimeError> {
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
            world,
            content,
            road,
        })
    }
    /// Authoritative saved state; mutable access is intentionally absent.
    pub const fn world(&self) -> &World {
        &self.world
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
    /// Release ownership of the unchanged authoritative world.
    pub fn into_world(self) -> World {
        self.world
    }
}
