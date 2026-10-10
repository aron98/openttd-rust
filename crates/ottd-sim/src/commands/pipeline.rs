mod accounting;
use super::{
    Command, CommandCost, CommandError, CommandGate, CommandMode, CommandReceipt, CommandRequest,
    CommandReturn, CommandReturnPhases, level_land, terrain_read::TerrainRead,
};
use crate::world_access::{signed, unsigned};
pub(super) use accounting::completion_edits;
use ottd_save::world::World;

/// Run native Post gates, body test, affordability and execution bookkeeping.
///
/// # Errors
/// Returns a scope error for unsupported gameplay or invalid saved state.
/// Rejected contexts leave the world unchanged.
pub fn execute_command(
    world: &mut World,
    request: &CommandRequest,
) -> Result<CommandReceipt, CommandError> {
    execute(world, request, None)
}
impl crate::runtime::RoadVehicleContext<'_> {
    pub(crate) fn execute(
        self,
        world: &mut World,
        request: &CommandRequest,
    ) -> Result<CommandReceipt, CommandError> {
        execute(world, request, Some(OwnedContext::RoadVehicle(self)))
    }
}
enum OwnedContext<'a> {
    RoadVehicle(crate::runtime::RoadVehicleContext<'a>),
    Depot(crate::runtime::DepotContext<'a>),
}
impl crate::runtime::DepotContext<'_> {
    pub(crate) fn execute(
        self,
        world: &mut World,
        request: &CommandRequest,
    ) -> Result<CommandReceipt, CommandError> {
        execute(world, request, Some(OwnedContext::Depot(self)))
    }
}
fn execute(
    world: &mut World,
    request: &CommandRequest,
    context: Option<OwnedContext<'_>>,
) -> Result<CommandReceipt, CommandError> {
    let tile = tile(&request.command);
    let tuple = matches!(
        request.command,
        Command::TerraformLand { .. } | Command::LevelLand { .. }
    );
    if tile != 0
        && world
            .map()
            .tiles()
            .get(usize::try_from(tile).map_err(|_| CommandError::Overflow("tile index"))?)
            .is_none_or(|t| !tuple && t.tile_type() >> 4 == 7)
    {
        return Ok(gated(CommandGate::Tile, request));
    }
    execute_valid_tile(world, request, tile, context)
}
const fn tile(command: &Command) -> u32 {
    match command {
        Command::BuildRoadDepot { tile, .. }
        | Command::SellVehicle { location: tile, .. }
        | Command::BuildRoad { tile, .. }
        | Command::BuildVehicle { tile, .. }
        | Command::LandscapeClear { tile }
        | Command::TerraformLand { tile, .. }
        | Command::LevelLand { tile, .. } => *tile,
        Command::ChangeServiceInterval { .. }
        | Command::IncreaseLoan { .. }
        | Command::DecreaseLoan { .. }
        | Command::RenameCompany { .. }
        | Command::RenamePresident { .. }
        | Command::Pause { .. } => 0,
    }
}
fn execute_valid_tile(
    world: &mut World,
    request: &CommandRequest,
    tile: u32,
    context: Option<OwnedContext<'_>>,
) -> Result<CommandReceipt, CommandError> {
    let server = matches!(request.command, Command::Pause { .. });
    let tuple = matches!(
        request.command,
        Command::TerraformLand { .. } | Command::LevelLand { .. }
    );
    let mut returns = (tuple || matches!(request.command, Command::BuildVehicle { .. }))
        .then(CommandReturnPhases::default);
    let estimate = request.mode == CommandMode::Estimate && !server;
    let pause = unsigned(world, b"DATE", 0, "pause_mode")?;
    if pause != 0
        && !estimate
        && unsigned(world, b"PATS", 0, "construction.command_pause_level")?
            < pause_level(&request.command)
    {
        return Ok(gated(CommandGate::Pause, request));
    }
    let company_exists = world
        .tables()
        .get(b"PLYR")
        .is_some_and(|t| t.records().contains_key(&u32::from(request.company)));
    if request.company == 18
        && matches!(
            request.command,
            Command::BuildRoad { .. } | Command::LandscapeClear { .. }
        )
    {
        return Err(CommandError::Unsupported("deity construction"));
    }
    if !server && !company_exists {
        if let Some(values) = &mut returns {
            values.result = Some(if matches!(request.command, Command::BuildVehicle { .. }) {
                CommandReturn::Vehicle {
                    vehicle: 0,
                    capacity: 0,
                    mail_capacity: 0,
                    cargo_capacities: Box::new(super::CargoCapacities([0; 64])),
                }
            } else {
                CommandReturn::Landscape {
                    additional_money: 0,
                    tile: 0,
                }
            });
        }
        return Ok(CommandReceipt {
            returns,
            posted: false,
            gate: None,
            test: None,
            exec: None,
            result: Some(CommandCost::failure("CMD_ERROR")),
        });
    }
    execute_admitted(
        world,
        request,
        tile,
        context,
        estimate,
        returns,
        company_exists,
    )
}
fn execute_admitted(
    world: &mut World,
    request: &CommandRequest,
    tile: u32,
    context: Option<OwnedContext<'_>>,
    estimate: bool,
    returns: Option<CommandReturnPhases>,
    company_exists: bool,
) -> Result<CommandReceipt, CommandError> {
    if matches!(request.command, Command::BuildRoadDepot { .. })
        || (matches!(request.command, Command::LandscapeClear { .. })
            && matches!(context, Some(OwnedContext::Depot(_))))
    {
        return execute_depot(world, request, estimate, context);
    }
    if let Command::BuildVehicle {
        tile,
        engine,
        cargo,
        use_free_vehicles: _,
        client_id: _,
    } = request.command
    {
        return super::vehicle_build::run(
            world,
            request.company,
            super::vehicle_build::Args {
                tile,
                engine,
                cargo,
            },
            estimate,
            match context {
                Some(OwnedContext::RoadVehicle(context)) => context,
                _ => {
                    return Err(CommandError::Unsupported(
                        "vehicle construction needs owned runtime",
                    ));
                }
            },
        );
    }
    if let Command::SellVehicle {
        location,
        vehicle,
        sell_chain: _,
        backup_order,
        client_id,
    } = request.command
    {
        return super::vehicle_sale::run(
            world,
            request.company,
            super::vehicle_sale::Args {
                location,
                vehicle,
                backup_order,
                client_id: if client_id == 0 { 1 } else { client_id },
            },
            estimate,
            match context {
                Some(OwnedContext::RoadVehicle(context)) => context,
                _ => {
                    return Err(CommandError::Unsupported(
                        "vehicle sale needs owned runtime",
                    ));
                }
            },
        );
    }
    if let Command::LevelLand {
        tile,
        start_tile,
        diagonal,
        level_mode,
    } = request.command
    {
        return level_land::run(
            world,
            request.company,
            level_land::Args {
                tile,
                start: start_tile,
                diagonal,
                mode: level_mode,
            },
            estimate,
        );
    }
    execute_planned(world, request, tile, estimate, returns, company_exists)
}
fn execute_depot(
    world: &mut World,
    request: &CommandRequest,
    estimate: bool,
    context: Option<OwnedContext<'_>>,
) -> Result<CommandReceipt, CommandError> {
    let Some(OwnedContext::Depot(context)) = context else {
        return Err(CommandError::Unsupported(
            "depot construction needs owned runtime",
        ));
    };
    match request.command {
        Command::LandscapeClear { tile } => {
            super::road_depot::remove::run(world, request.company, tile, estimate, context)
        }
        Command::BuildRoadDepot {
            tile,
            road_type,
            direction,
        } => super::road_depot::run(
            world,
            request.company,
            super::road_depot::Args {
                tile,
                road_type,
                direction,
            },
            estimate,
            context,
        ),
        _ => Err(CommandError::Unsupported("depot command context")),
    }
}
fn execute_planned(
    world: &mut World,
    request: &CommandRequest,
    tile: u32,
    estimate: bool,
    mut returns: Option<CommandReturnPhases>,
    company_exists: bool,
) -> Result<CommandReceipt, CommandError> {
    let plan = super::body(world, request)?;
    if let Some(values) = &mut returns {
        values.test.clone_from(&plan.returns);
        values.result.clone_from(&plan.returns);
    }
    let test = plan.cost.clone();
    let mut result = test.clone();
    if !result.success || estimate {
        return Ok(CommandReceipt {
            returns,
            posted: result.success,
            gate: None,
            test: Some(test),
            exec: None,
            result: Some(result),
        });
    }
    let company = u32::from(request.company);
    if result.cost > 0
        && company_exists
        && unsigned(world, b"PATS", 0, "difficulty.infinite_money")? == 0
        && result.cost > signed(world, b"PLYR", company, "money")?
    {
        result.success = false;
        result.error = Some("STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY".into());
        result.error_params = vec![result.cost];
        return Ok(CommandReceipt {
            returns,
            posted: false,
            gate: None,
            test: Some(test),
            exec: None,
            result: Some(result),
        });
    }
    publish(world, request, tile, plan, test, result, returns)
}

fn publish(
    world: &mut World,
    request: &CommandRequest,
    tile: u32,
    plan: super::Plan,
    test: CommandCost,
    result: CommandCost,
    mut returns: Option<CommandReturnPhases>,
) -> Result<CommandReceipt, CommandError> {
    let server = matches!(request.command, Command::Pause { .. });
    let company = u32::from(request.company);
    let mut edits = plan.edits;
    if !server {
        edits.extend(completion_edits(
            TerrainRead::Committed(world),
            company,
            tile,
            &result,
        )?);
    }
    world.edit_batch(edits)?;
    if let Some(values) = &mut returns {
        values.exec = plan.returns;
    }
    Ok(CommandReceipt {
        returns,
        posted: true,
        gate: None,
        test: Some(test),
        exec: Some(result.clone()),
        result: Some(result),
    })
}
fn gated(gate: CommandGate, request: &CommandRequest) -> CommandReceipt {
    CommandReceipt {
        returns: matches!(
            request.command,
            Command::TerraformLand { .. }
                | Command::LevelLand { .. }
                | Command::BuildVehicle { .. }
        )
        .then(CommandReturnPhases::default),
        posted: false,
        gate: Some(gate),
        test: None,
        exec: None,
        result: None,
    }
}

const fn pause_level(command: &Command) -> u64 {
    match command {
        Command::ChangeServiceInterval { .. } => 1,
        Command::BuildRoadDepot { .. }
        | Command::BuildRoad { .. }
        | Command::LandscapeClear { .. }
        | Command::TerraformLand { .. }
        | Command::LevelLand { .. } => 3,
        Command::IncreaseLoan { .. }
        | Command::DecreaseLoan { .. }
        | Command::BuildVehicle { .. }
        | Command::SellVehicle { .. } => 2,
        Command::RenameCompany { .. } | Command::RenamePresident { .. } | Command::Pause { .. } => {
            0
        }
    }
}
