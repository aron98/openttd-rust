use super::{
    Command, CommandCost, CommandError, CommandGate, CommandMode, CommandReceipt, CommandRequest,
    CommandReturn, CommandReturnPhases,
};
use crate::world_access::{field, field_edit, signed, unsigned};
use ottd_save::{
    WireValue,
    world::{PathElement, World, WorldEdit},
};

/// Run native Post gates, body test, affordability and execution bookkeeping.
///
/// # Errors
/// Returns a scope error for unsupported gameplay or invalid saved state.
/// Rejected contexts leave the world unchanged.
pub fn execute_command(
    world: &mut World,
    request: &CommandRequest,
) -> Result<CommandReceipt, CommandError> {
    let tile = tile(&request.command);
    let tuple = matches!(request.command, Command::TerraformLand { .. });
    if tile != 0
        && world
            .map()
            .tiles()
            .get(usize::try_from(tile).map_err(|_| CommandError::Overflow("tile index"))?)
            .is_none_or(|t| !tuple && t.tile_type() >> 4 == 7)
    {
        return Ok(gated(CommandGate::Tile, tuple));
    }
    execute_valid_tile(world, request, tile)
}
const fn tile(command: &Command) -> u32 {
    match command {
        Command::BuildRoad { tile, .. }
        | Command::LandscapeClear { tile }
        | Command::TerraformLand { tile, .. } => *tile,
        Command::IncreaseLoan { .. }
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
) -> Result<CommandReceipt, CommandError> {
    let server = matches!(request.command, Command::Pause { .. });
    let tuple = matches!(request.command, Command::TerraformLand { .. });
    let mut returns = tuple.then(CommandReturnPhases::default);
    let estimate = request.mode == CommandMode::Estimate && !server;
    let pause = unsigned(world, b"DATE", 0, "pause_mode")?;
    let required_level = match request.command {
        Command::BuildRoad { .. }
        | Command::LandscapeClear { .. }
        | Command::TerraformLand { .. } => 3,
        Command::IncreaseLoan { .. } | Command::DecreaseLoan { .. } => 2,
        Command::RenameCompany { .. } | Command::RenamePresident { .. } | Command::Pause { .. } => {
            0
        }
    };
    if pause != 0
        && !estimate
        && unsigned(world, b"PATS", 0, "construction.command_pause_level")? < required_level
    {
        return Ok(gated(CommandGate::Pause, tuple));
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
            values.result = Some(CommandReturn::Landscape {
                additional_money: 0,
                tile: 0,
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
    let plan = super::body(world, request)?;
    if let Some(values) = &mut returns {
        values.test = plan.returns;
        values.result = plan.returns;
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
    let pause = unsigned(world, b"DATE", 0, "pause_mode")?;
    let mut edits = plan.edits;
    if !server {
        if tile != 0 {
            edits.push(field_edit(
                *b"PLYR",
                company,
                "last_build_coordinate",
                WireValue::Unsigned(u64::from(tile)),
            ));
        }
        edits.extend(accounting(world, company, &result)?);
    }
    if pause != 0 && !server {
        edits.push(field_edit(
            *b"DATE",
            0,
            "pause_mode",
            WireValue::Unsigned(pause | 128),
        ));
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
fn accounting(
    world: &World,
    company: u32,
    result: &CommandCost,
) -> Result<Vec<WorldEdit>, CommandError> {
    let mut edits = Vec::new();
    if result.cost != 0 {
        let money = signed(world, b"PLYR", company, "money")?.saturating_sub(result.cost);
        edits.push(field_edit(
            *b"PLYR",
            company,
            "money",
            WireValue::Signed(money),
        ));
        let WireValue::Array(expenses) = field(world, b"PLYR", company, "yearly_expenses")? else {
            return Err(CommandError::Unsupported("yearly expense wire layout"));
        };
        let index = usize::from(result.expenses);
        let Some(WireValue::Signed(expense)) = expenses.get(index) else {
            return Err(CommandError::Unsupported("expense category"));
        };
        edits.push(WorldEdit::Field {
            chunk: *b"PLYR",
            record: company,
            path: vec![
                PathElement::Field("yearly_expenses".into()),
                PathElement::Index(index),
            ],
            value: WireValue::Signed(expense.saturating_add(result.cost)),
        });
    }
    Ok(edits)
}
fn gated(gate: CommandGate, tuple: bool) -> CommandReceipt {
    CommandReceipt {
        returns: tuple.then(CommandReturnPhases::default),
        posted: false,
        gate: Some(gate),
        test: None,
        exec: None,
        result: None,
    }
}
