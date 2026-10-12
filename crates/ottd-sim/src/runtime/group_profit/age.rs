//! Exact minimum-age contribution; the all-age sum remains unchanged.
use super::{GroupProfitCaches, ProfitFields, RuntimeError, SavedVehicleView};
impl ProfitFields {
    pub(super) const fn reached_min_age(&mut self, display_profit: i64) {
        self.num_vehicle_min_age = self.num_vehicle_min_age.wrapping_add(1);
        self.profit_last_year_min_age =
            self.profit_last_year_min_age.saturating_add(display_profit);
    }
}
impl GroupProfitCaches {
    pub(in crate::runtime) fn reached_min_age(
        &mut self,
        vehicle: SavedVehicleView<'_>,
    ) -> Result<(), RuntimeError> {
        let owner = vehicle.owner()?;
        let group = u16::try_from(vehicle.common_number("group_id")?)
            .map_err(|_| RuntimeError::Invalid("group ID"))?;
        let display_profit = vehicle.common_signed("profit_last_year")? >> 8;
        let (all, target) = self.road_groups(owner, group)?;
        all.reached_min_age(display_profit);
        target.reached_min_age(display_profit);
        Ok(())
    }
}
