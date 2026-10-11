// SPDX-License-Identifier: GPL-2.0-only
//! Native type membership restored by `industry_sl.cpp` and map-scaled daily scheduling.
use super::RuntimeError;
use crate::world_access::unsigned;
use ottd_save::world::World;
use serde::Serialize;

/// Simulation membership and scheduling caches; recipient catchments are a separate consumer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndustryCaches {
    /// Sparse industry IDs in native pool order for each of the 240 industry types.
    pub type_members: Vec<Vec<u16>>,
    /// Native map-area divided by 31, restored by `InitializeIndustryDailyChanges`.
    pub daily_increment: u32,
}

impl IndustryCaches {
    pub(crate) fn restore(world: &World) -> Result<Self, RuntimeError> {
        let mut type_members = vec![Vec::new(); 240];
        let table = world
            .tables()
            .get(b"INDY")
            .ok_or(RuntimeError::Invalid("INDY"))?;
        for id in table.records().keys() {
            let kind = usize::try_from(
                unsigned(world, b"INDY", *id, "type")
                    .map_err(|_| RuntimeError::Invalid("industry type"))?,
            )
            .map_err(|_| RuntimeError::Invalid("industry type"))?;
            type_members
                .get_mut(kind)
                .ok_or(RuntimeError::Invalid("industry type"))?
                .push(u16::try_from(*id).map_err(|_| RuntimeError::Invalid("industry ID"))?);
        }
        Ok(Self {
            type_members,
            daily_increment: world.map().width().saturating_mul(world.map().height()) / 31,
        })
    }
}
