use super::{
    RuntimeError, SavedVehicleView, VehicleId,
    pools::{PoolAllocator, RoadUnitNumbers, UnitNumberAllocator, VehicleType},
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
        let mut freeunits = BTreeMap::<(u8, VehicleType), UnitNumberAllocator>::new();
        for id in table.records().keys() {
            let vehicle = SavedVehicleView::new(world, VehicleId::new(*id))?;
            // Native Company::freeunits[v->type].UseID(v->unitnumber) at
            // saveload/vehicle_sl.cpp:538; the saved view guarantees VEH_ROAD,
            // and every other family fails restoration before reaching here.
            freeunits
                .entry((vehicle.owner()?, VehicleType::Road))
                .or_default()
                .use_id(vehicle.unit_number()?);
        }
        Ok(Self { pool, freeunits })
    }

    /// Native per-company road unit-number state; companies without vehicles report 1.
    ///
    /// `count` mirrors native `group_all[company][VEH_ROAD].num_vehicle`: that
    /// field is NOSAVE (group.h:80), so native load recomputes it from vehicle
    /// records via `GroupStatistics::UpdateAfterLoad` (group_cmd.cpp:120-145,
    /// called at afterload.cpp:297), counting every `IsEngineCountable` and
    /// `IsPrimaryVehicle` vehicle, which for `VEH_ROAD` is exactly the front
    /// engines (vehicle.cpp:692, roadveh.h:122). Restoration admits only
    /// single-part front road vehicles (`saved_vehicle.rs` rejects other
    /// families and non-front parts), so the live count of each company's
    /// restored vehicle records equals the native recomputation.
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
                // Native uint16_t num_vehicle wraps past 65535 owned vehicles.
                let owned = super::road_company_count(world, company)?;
                Ok(RoadUnitNumbers {
                    company,
                    count: u16::try_from(owned & u64::from(u16::MAX))
                        .map_err(|_| RuntimeError::Invalid("vehicle count"))?,
                    next: self
                        .freeunits
                        .get(&(company, VehicleType::Road))
                        .map_or(1, UnitNumberAllocator::next_id),
                })
            })
            .collect()
    }
}
