//! Owned fields changed by native `GroupStatistics::UpdateProfits` and age731.
mod restore;
#[cfg(test)]
mod tests;
use super::{RuntimeError, SavedVehicleView};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
struct ProfitFields {
    profit_last_year: i64,
    profit_last_year_min_age: i64,
    num_vehicle_min_age: u16,
}
impl ProfitFields {
    const fn add(&mut self, display_profit: i64, eligible: bool) {
        self.profit_last_year = self.profit_last_year.saturating_add(display_profit);
        if eligible {
            self.profit_last_year_min_age =
                self.profit_last_year_min_age.saturating_add(display_profit);
            self.num_vehicle_min_age = self.num_vehicle_min_age.wrapping_add(1);
        }
    }
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
struct CompanyProfits {
    all: [ProfitFields; 4],
    #[serde(rename = "default")]
    default_group: [ProfitFields; 4],
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct NamedProfit {
    owner: u8,
    vehicle_type: u8,
    parent: u16,
    profit: ProfitFields,
}

/// Exactly the three native group-profit fields, separate from engine/count/renewal caches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GroupProfitCaches {
    companies: BTreeMap<u8, CompanyProfits>,
    named: BTreeMap<u16, NamedProfit>,
}
impl GroupProfitCaches {
    fn add_vehicle(&mut self, vehicle: SavedVehicleView<'_>) -> Result<(), RuntimeError> {
        self.add_road(
            vehicle.owner()?,
            u16::try_from(vehicle.common_number("group_id")?)
                .map_err(|_| RuntimeError::Invalid("group ID"))?,
            vehicle.common_signed("profit_last_year")? >> 8,
            vehicle.common_number("economy_age")? > 730,
        )
    }
    fn add_road(
        &mut self,
        owner: u8,
        group: u16,
        display_profit: i64,
        eligible: bool,
    ) -> Result<(), RuntimeError> {
        let company = self
            .companies
            .get_mut(&owner)
            .ok_or(RuntimeError::Invalid("group company"))?;
        let target = if group == 65534 {
            company
                .default_group
                .get_mut(1)
                .ok_or(RuntimeError::Invalid("road group type"))?
        } else {
            let named = self
                .named
                .get_mut(&group)
                .ok_or(RuntimeError::Invalid("vehicle group"))?;
            if named.owner != owner || named.vehicle_type != 1 {
                return Err(RuntimeError::Invalid("vehicle group owner/type"));
            }
            &mut named.profit
        };
        company
            .all
            .get_mut(1)
            .ok_or(RuntimeError::Invalid("road group type"))?
            .add(display_profit, eligible);
        target.add(display_profit, eligible);
        Ok(())
    }
}
