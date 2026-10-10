use super::{BackupAllocation, OrderState, RuntimeError, RuntimeSaveContext, VehicleId, backup};
use ottd_save::world::{PreparedWorldTransaction, World, WorldEdit, WorldTransaction};

#[derive(Debug)]
pub(in crate::runtime) struct SaleBackupPlan {
    edits: Vec<WorldEdit>,
    backups: BackupAllocation,
    vehicle: VehicleId,
    replacement: Option<u32>,
}
#[derive(Debug)]
pub(in crate::runtime) struct PendingSaleBackups {
    backups: BackupAllocation,
}
#[derive(Debug)]
pub(in crate::runtime) struct PreparedSaleBackups {
    backups: BackupAllocation,
}
impl OrderState {
    pub(in crate::runtime) const fn context(&self) -> RuntimeSaveContext {
        self.context
    }
    pub(in crate::runtime) fn plan_sale_backups(
        &self,
        world: &World,
        id: VehicleId,
        backup_user: Option<u32>,
    ) -> Result<SaleBackupPlan, RuntimeError> {
        let ids = super::view::table(world, *b"BKOR")?
            .records()
            .keys()
            .copied()
            .collect::<Vec<_>>();
        self.backups.validate_slots(&ids)?;
        let replacement = backup::clone_replacement(world, id)?;
        let (edits, backups) = match backup_user {
            Some(user) => backup::create(self, world, id, user)?,
            None => (Vec::new(), self.backups.clone()),
        };
        Ok(SaleBackupPlan {
            edits,
            backups,
            vehicle: id,
            replacement,
        })
    }
}
impl SaleBackupPlan {
    pub(in crate::runtime) fn stage(
        self,
        transaction: &mut WorldTransaction<'_>,
    ) -> Result<PendingSaleBackups, RuntimeError> {
        for edit in self.edits {
            transaction.apply(edit)?;
        }
        let (edits, backups) = backup::clear_vehicle_from(
            &self.backups,
            super::view::OrderReader::Candidate(transaction.view()),
            self.vehicle,
            self.replacement,
        )?;
        for edit in edits {
            transaction.apply(edit)?;
        }
        Ok(PendingSaleBackups { backups })
    }
}
impl PendingSaleBackups {
    pub(in crate::runtime) fn validate(
        self,
        prepared: &PreparedWorldTransaction<'_>,
    ) -> Result<PreparedSaleBackups, RuntimeError> {
        let ids = prepared
            .view()
            .table(*b"BKOR")
            .ok_or(RuntimeError::Invalid("BKOR"))?
            .records()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        self.backups.validate_slots(&ids)?;
        Ok(PreparedSaleBackups {
            backups: self.backups,
        })
    }
}
impl PreparedSaleBackups {
    pub(in crate::runtime) fn publish(self, state: &mut OrderState) {
        state.backups = self.backups;
    }
}
