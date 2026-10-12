//! Candidate-only annual recount and prepublication consistency check.
use super::{GroupProfitCaches, ProfitFields};
use crate::runtime::{RuntimeError, SavedVehicleView, VehicleId};
use ottd_save::world::CandidateView;

impl GroupProfitCaches {
    pub(in crate::runtime) fn recount(
        &mut self,
        view: CandidateView<'_>,
    ) -> Result<(), RuntimeError> {
        for company in self.companies.values_mut() {
            company.all.fill(ProfitFields::default());
            company.default_group.fill(ProfitFields::default());
        }
        for group in self.named.values_mut() {
            group.profit = ProfitFields::default();
        }
        for (id, _) in view
            .table(*b"VEHS")
            .ok_or(RuntimeError::Invalid("VEHS"))?
            .records()
        {
            self.add_vehicle(SavedVehicleView::candidate(view, VehicleId::new(id))?)?;
        }
        Ok(())
    }

    pub(in crate::runtime) fn validate(&self, view: CandidateView<'_>) -> Result<(), RuntimeError> {
        let mut expected = self.clone();
        expected.recount(view)?;
        if *self != expected {
            return Err(RuntimeError::Invalid("candidate group profits"));
        }
        Ok(())
    }
}
