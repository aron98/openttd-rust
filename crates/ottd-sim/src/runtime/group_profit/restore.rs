//! Native afterload contribution order, restricted to the runtime's admitted road fronts.
use super::{CompanyProfits, GroupProfitCaches, NamedProfit, ProfitFields};
use crate::{
    runtime::{RuntimeError, SavedVehicleView, VehicleId},
    world_access::unsigned,
};
use ottd_save::world::World;
use std::collections::BTreeMap;

impl GroupProfitCaches {
    pub(crate) fn restore(world: &World) -> Result<Self, RuntimeError> {
        let mut companies = BTreeMap::new();
        for id in world
            .tables()
            .get(b"PLYR")
            .ok_or(RuntimeError::Invalid("PLYR"))?
            .records()
            .keys()
        {
            companies.insert(
                u8::try_from(*id).map_err(|_| RuntimeError::Invalid("group company ID"))?,
                CompanyProfits::default(),
            );
        }
        let mut named = BTreeMap::new();
        for id in world
            .tables()
            .get(b"GRPS")
            .ok_or(RuntimeError::Invalid("GRPS"))?
            .records()
            .keys()
        {
            let field =
                |name| unsigned(world, b"GRPS", *id, name).map_err(|_| RuntimeError::Invalid(name));
            let owner =
                u8::try_from(field("owner")?).map_err(|_| RuntimeError::Invalid("group owner"))?;
            let vehicle_type = u8::try_from(field("vehicle_type")?)
                .map_err(|_| RuntimeError::Invalid("group type"))?;
            if !companies.contains_key(&owner) || vehicle_type >= 4 {
                return Err(RuntimeError::Invalid("group owner/type"));
            }
            named.insert(
                u16::try_from(*id).map_err(|_| RuntimeError::Invalid("group ID"))?,
                NamedProfit {
                    owner,
                    vehicle_type,
                    parent: u16::try_from(field("parent")?)
                        .map_err(|_| RuntimeError::Invalid("group parent"))?,
                    profit: ProfitFields::default(),
                },
            );
        }
        let mut cache = Self { companies, named };
        for id in world
            .tables()
            .get(b"VEHS")
            .ok_or(RuntimeError::Invalid("VEHS"))?
            .records()
            .keys()
        {
            cache.add_vehicle(SavedVehicleView::new(world, VehicleId::new(*id))?)?;
        }
        Ok(cache)
    }
}
