use super::{RuntimeError, SavedVehicleView, SimulationRuntime, VehicleId};
use crate::content::VehicleSpec;
use serde::Serialize;
use std::collections::BTreeMap;

/// Road vehicle and engine counts; excludes profit and autoreplace statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoadGroupCounts {
    /// Native wrapping vehicle count.
    pub vehicles: u16,
    /// Count for every catalog road engine, including zero counts.
    pub engines: BTreeMap<u16, u16>,
}
/// All-group and ungrouped road counts for one company.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompanyRoadCounts {
    /// Includes named-group vehicles as well as ungrouped vehicles.
    pub all: RoadGroupCounts,
    /// Only vehicles in native `DEFAULT_GROUP`.
    pub default_group: RoadGroupCounts,
}
impl RoadGroupCounts {
    fn count(&mut self, engine: u16) -> Result<(), RuntimeError> {
        self.vehicles = self.vehicles.wrapping_add(1);
        let count = self
            .engines
            .get_mut(&engine)
            .ok_or(RuntimeError::Invalid("group engine"))?;
        *count = count.wrapping_add(1);
        Ok(())
    }
}
impl SimulationRuntime {
    /// Read all/default road counts from the authoritative live vehicle records.
    ///
    /// # Errors
    /// Rejects unsupported vehicles or invalid owners/engine fields.
    pub fn road_group_counts(&self) -> Result<BTreeMap<u8, CompanyRoadCounts>, RuntimeError> {
        let empty = RoadGroupCounts {
            vehicles: 0,
            engines: self
                .content
                .engines()
                .iter()
                .filter(|e| matches!(e.vehicle, VehicleSpec::Road(_)))
                .map(|e| (e.id, 0))
                .collect(),
        };
        let mut groups = BTreeMap::new();
        for id in self
            .world
            .tables()
            .get(b"PLYR")
            .ok_or(RuntimeError::Invalid("PLYR"))?
            .records()
            .keys()
        {
            groups.insert(
                u8::try_from(*id).map_err(|_| RuntimeError::Invalid("company"))?,
                CompanyRoadCounts {
                    all: empty.clone(),
                    default_group: empty.clone(),
                },
            );
        }
        for id in self
            .world
            .tables()
            .get(b"VEHS")
            .ok_or(RuntimeError::Invalid("VEHS"))?
            .records()
            .keys()
        {
            let vehicle = SavedVehicleView::new(&self.world, VehicleId::new(*id))?;
            let company = groups
                .get_mut(&vehicle.owner()?)
                .ok_or(RuntimeError::Invalid("vehicle owner"))?;
            company.all.count(vehicle.engine_id()?)?;
            if vehicle.common_number("group_id")? == 65534 {
                company.default_group.count(vehicle.engine_id()?)?;
            }
        }
        Ok(groups)
    }
}
