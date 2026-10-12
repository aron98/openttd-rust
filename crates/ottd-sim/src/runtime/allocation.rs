use super::{
    RuntimeError, SavedVehicleView, VehicleId,
    pools::{PoolAllocator, RoadUnitNumbers, UnitNumberAllocator},
};
use ottd_save::world::World;
use std::collections::BTreeMap;

use super::VehicleAllocation;
impl VehicleAllocation {
    pub(super) fn restore(world: &World) -> Result<Self, RuntimeError> {
        let table = world
            .tables()
            .get(b"VEHS")
            .ok_or(RuntimeError::Invalid("VEHS"))?;
        let pool = PoolAllocator::restore(0xFF000, 512, table.records().keys().copied())?;
        let mut road_units = BTreeMap::<u8, UnitNumberAllocator>::new();
        for id in table.records().keys() {
            let vehicle = SavedVehicleView::new(world, VehicleId::new(*id))?;
            road_units
                .entry(vehicle.owner()?)
                .or_default()
                .use_id(vehicle.unit_number()?);
        }
        Ok(Self { pool, road_units })
    }

    /// Native per-company road unit-number state; companies without vehicles report 1.
    ///
    /// # Errors
    /// Rejects invalid saved company IDs.
    pub(super) fn road_unit_numbers(
        &self,
        world: &World,
    ) -> Result<Vec<RoadUnitNumbers>, RuntimeError> {
        world
            .tables()
            .get(b"PLYR")
            .ok_or(RuntimeError::Invalid("PLYR"))?
            .records()
            .keys()
            .map(|id| {
                let company = u8::try_from(*id).map_err(|_| RuntimeError::Invalid("company ID"))?;
                Ok(RoadUnitNumbers {
                    company,
                    next: self
                        .road_units
                        .get(&company)
                        .map_or(1, UnitNumberAllocator::next_id),
                })
            })
            .collect()
    }
}
