use super::{SavedVehicleView, VehicleId, World, WorldTickError, unsupported};
use std::collections::BTreeMap;

pub(super) struct Spatial {
    buckets: BTreeMap<u32, Vec<VehicleId>>,
    current: u32,
}
const fn bucket(tile: u32, width: u32) -> u32 {
    ((tile & width.wrapping_sub(1)) & 127) | (((tile >> width.trailing_zeros()) & 127) << 7)
}
impl Spatial {
    pub(super) fn restore(world: &World, id: VehicleId) -> Result<Self, WorldTickError> {
        if world
            .tables()
            .get(b"VEHS")
            .is_none_or(|t| t.records().len() != 1)
        {
            return Err(unsupported("spatial", "single-vehicle restoration only"));
        }
        let current = bucket(
            SavedVehicleView::new(world, id)?.tile()?,
            world.map().width(),
        );
        Ok(Self {
            buckets: BTreeMap::from([(current, vec![id])]),
            current,
        })
    }
    pub(super) fn update(
        &mut self,
        vehicle: SavedVehicleView<'_>,
        width: u32,
    ) -> Result<(), WorldTickError> {
        let next = bucket(vehicle.tile()?, width);
        if next == self.current {
            return Ok(());
        }
        let old = self
            .buckets
            .remove(&self.current)
            .ok_or_else(|| unsupported("spatial", "missing old bucket"))?;
        if old != [vehicle.id()] {
            return Err(unsupported("spatial", "unexpected chain"));
        }
        self.buckets
            .entry(next)
            .or_default()
            .insert(0, vehicle.id());
        self.current = next;
        Ok(())
    }
    pub(super) fn ensure_unoccupied(
        &self,
        tile: i64,
        id: VehicleId,
        width: u32,
    ) -> Result<(), WorldTickError> {
        let tile = u32::try_from(tile).map_err(|_| unsupported("spatial", "tile"))?;
        if self
            .buckets
            .get(&bucket(tile, width))
            .is_some_and(|ids| ids.iter().any(|other| *other != id))
        {
            return Err(unsupported("spatial", "other vehicle collision"));
        }
        Ok(())
    }
    pub(super) fn validate(
        &self,
        vehicle: SavedVehicleView<'_>,
        width: u32,
    ) -> Result<(), WorldTickError> {
        let current = bucket(vehicle.tile()?, width);
        if current != self.current
            || self.buckets.len() != 1
            || self
                .buckets
                .get(&current)
                .is_none_or(|ids| ids.as_slice() != [vehicle.id()])
        {
            return Err(unsupported("spatial", "candidate index mismatch"));
        }
        Ok(())
    }
}

/// Gameplay tile buckets reconstructible for exactly one saved road vehicle.
/// This excludes sprite bounds and viewport buckets.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SingleRoadTileOccupancy {
    /// Native 7-bit-per-axis tile hash bucket.
    pub bucket: u32,
    /// Sole native vehicle identity.
    pub vehicle: VehicleId,
}
impl super::SimulationRuntime {
    /// Reads single-vehicle gameplay occupancy from canonical saved positions.
    /// # Errors
    /// Refuses multiple vehicles because their chain history is not saved.
    pub fn single_road_tile_occupancy(&self) -> Result<SingleRoadTileOccupancy, WorldTickError> {
        let (id, _) = self
            .road_caches()
            .first_key_value()
            .ok_or_else(|| unsupported("spatial", "missing vehicle"))?;
        let spatial = Spatial::restore(self.world(), *id)?;
        Ok(SingleRoadTileOccupancy {
            bucket: spatial.current,
            vehicle: *id,
        })
    }
}
