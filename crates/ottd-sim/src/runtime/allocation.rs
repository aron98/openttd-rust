use super::{
    RuntimeError, SavedVehicleView, VehicleId,
    pools::{PoolAllocator, UnitNumberAllocator},
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
}
