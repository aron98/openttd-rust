//! Dispatch runtime commands through their existing transactional contexts.
use super::{
    DepotContext, OrderContext, RoadVehicleContext, RuntimeSaveContext, SimulationRuntime,
    StationContext,
};

impl SimulationRuntime {
    /// Execute admitted service and road-purchase commands over the owned world.
    /// # Errors
    /// Rejects unsupported contexts and commands without live cache publication.
    pub fn execute_command(
        &mut self,
        request: &crate::CommandRequest,
    ) -> Result<crate::CommandReceipt, crate::CommandError> {
        if matches!(request.command, crate::Command::LandscapeClear { .. })
            && self.save_context() != RuntimeSaveContext::SinglePlayer
        {
            return Err(crate::CommandError::Unsupported(
                "depot removal host context",
            ));
        }
        match request.command {
            crate::Command::BuildRoadStop { .. } => StationContext {
                cargo: &mut self.transport.cargo,
                content: &self.content,
                stations: &mut self.stations,
                road: &mut self.depot.road,
            }
            .execute(&mut self.world, request),
            crate::Command::InsertOrder { .. } => {
                OrderContext::new(&mut self.orders, &self.content, &self.road)
                    .execute(&mut self.world, request)
            }
            crate::Command::BuildRoadDepot { .. } | crate::Command::LandscapeClear { .. } => {
                DepotContext {
                    orders: &mut self.orders,
                    content: &self.content,
                    pool: &mut self.depot.pool,
                    road: &mut self.depot.road,
                }
                .execute(&mut self.world, request)
            }
            crate::Command::BuildVehicle { .. } | crate::Command::SellVehicle { .. } => {
                RoadVehicleContext {
                    cargo: &mut self.transport.cargo,
                    orders: &mut self.orders,
                    serializer_cargo_paid_for: self.serializer_cargo_paid_for,
                    content: &self.content,
                    allocation: &mut self.allocation,
                    road: &mut self.road,
                }
                .execute(&mut self.world, request)
            }
            crate::Command::ChangeServiceInterval { .. } => {
                crate::commands::execute_command(&mut self.world, request)
            }
            crate::Command::LevelLand { .. }
            | crate::Command::TerraformLand { .. }
            | crate::Command::BuildRoad { .. }
            | crate::Command::IncreaseLoan { .. }
            | crate::Command::DecreaseLoan { .. }
            | crate::Command::RenameCompany { .. }
            | crate::Command::RenamePresident { .. }
            | crate::Command::Pause { .. } => Err(crate::CommandError::Unsupported(
                "runtime command cache publication",
            )),
        }
    }
}
