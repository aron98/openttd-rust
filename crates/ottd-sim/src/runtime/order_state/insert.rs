mod list;
mod publication;
mod station;
mod validation;
mod vehicles;
use super::{OrderState, RuntimeError};
use crate::{
    commands::{Command, CommandCost, CommandError, CommandReceipt, CommandRequest},
    content::ContentCatalog,
    runtime::{OrderContext, RoadVehicleCache, VehicleId},
};
use ottd_save::world::World;
use std::collections::BTreeMap;

impl<'a> OrderContext<'a> {
    pub(in crate::runtime) const fn new(
        state: &'a mut OrderState,
        content: &'a ContentCatalog,
        road: &'a BTreeMap<VehicleId, RoadVehicleCache>,
    ) -> Self {
        Self {
            state,
            content,
            road,
        }
    }
    pub(crate) fn insert(
        self,
        world: &mut World,
        request: &CommandRequest,
        estimate: bool,
    ) -> Result<CommandReceipt, CommandError> {
        let Command::InsertOrder {
            location,
            vehicle,
            index,
            order,
        } = request.command
        else {
            return Err(CommandError::Unsupported("order command context"));
        };
        let args = validation::Insertion {
            company: request.company,
            location,
            vehicle: VehicleId::new(vehicle),
            index,
            order,
        };
        let result = validation::validate(world, &self, &args)?;
        let executed = result.success && !estimate;
        if executed {
            publication::publish(world, self.state, &args)?;
        }
        Ok(CommandReceipt {
            posted: result.success,
            gate: None,
            test: Some(result.clone()),
            exec: executed.then(|| result.clone()),
            result: Some(result),
            returns: None,
        })
    }
}
