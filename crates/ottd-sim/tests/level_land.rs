//! `LevelLand` phase differences, partial completion and candidate rollback.
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World},
};
use ottd_sim::{Command, CommandMode, CommandRequest, CommandReturn, execute_command};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn world() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/replay/clear-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
fn field(world: &mut World, name: &str, value: WireValue) -> Result {
    world.edit_field(*b"PLYR", 0, &[PathElement::Field(name.into())], value)?;
    Ok(())
}
const fn request(tile: u32, start_tile: u32, level_mode: u8) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command: Command::LevelLand {
            tile,
            start_tile,
            diagonal: false,
            level_mode,
        },
    }
}
#[test]
fn raise_selection_executes_against_previously_cleared_tiles() -> Result {
    let mut world = world()?;
    let receipt = execute_command(&mut world, &request(652, 650, 2))?;
    assert!(receipt.posted);
    assert!(receipt.test.ok_or("test")?.cost > receipt.exec.ok_or("exec")?.cost);
    for index in [650, 651, 652] {
        assert_eq!(world.map().tiles().get(index).ok_or("tile")?.height(), 5);
    }
    Ok(())
}
#[test]
fn estimate_at_one_step_limit_succeeds_without_charging_or_mutating() -> Result {
    let mut world = world()?;
    field(&mut world, "terraform_limit", WireValue::Unsigned(65536))?;
    let before = world.saved_json()?;
    let mut command = request(652, 650, 2);
    command.mode = CommandMode::Estimate;
    let receipt = execute_command(&mut world, &command)?;
    assert!(receipt.posted);
    assert_eq!(receipt.result.ok_or("result")?.cost, 0);
    assert!(receipt.exec.is_none());
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
#[test]
fn zero_cash_enters_execution_and_returns_full_next_step_cost() -> Result {
    let mut world = world()?;
    field(&mut world, "money", WireValue::Signed(0))?;
    let before = world.saved_json()?;
    let receipt = execute_command(&mut world, &request(652, 650, 2))?;
    assert!(receipt.test.ok_or("test")?.success);
    assert_eq!(receipt.exec.ok_or("exec")?.cost, 0);
    let result = receipt.result.ok_or("result")?;
    assert_eq!(
        result.error.as_deref(),
        Some("STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY")
    );
    let Some(CommandReturn::Landscape {
        additional_money,
        tile,
    }) = receipt.returns.ok_or("returns")?.result
    else {
        return Err("tuple".into());
    };
    assert!(additional_money > 0);
    assert_eq!(result.error_params, vec![additional_money]);
    assert_eq!(tile, 650);
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
#[test]
fn invalid_raw_mode_returns_native_error_without_changes() -> Result {
    let mut world = world()?;
    let before = world.saved_json()?;
    let receipt = execute_command(&mut world, &request(652, 650, 255))?;
    assert_eq!(
        receipt.result.ok_or("result")?.error.as_deref(),
        Some("CMD_ERROR")
    );
    assert!(receipt.exec.is_none());
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
#[test]
fn exhausted_limit_rejects_already_level_selection() -> Result {
    let mut world = world()?;
    field(&mut world, "terraform_limit", WireValue::Unsigned(65535))?;
    let receipt = execute_command(&mut world, &request(652, 650, 0))?;
    assert_eq!(
        receipt.result.ok_or("result")?.error.as_deref(),
        Some("STR_ERROR_TERRAFORM_LIMIT_REACHED")
    );
    Ok(())
}

#[test]
fn limit_exhaustion_keeps_the_successful_execution_prefix() -> Result {
    let mut world = world()?;
    field(
        &mut world,
        "terraform_limit",
        WireValue::Unsigned(65536 + 17),
    )?;
    let receipt = execute_command(&mut world, &request(652, 650, 2))?;
    assert!(receipt.posted);
    assert_eq!(receipt.test.ok_or("test")?.cost, 0);
    assert!(receipt.exec.ok_or("exec")?.cost > 0);
    assert_eq!(world.map().tiles().get(650).ok_or("tile")?.height(), 5);
    assert_eq!(world.map().tiles().get(651).ok_or("tile")?.height(), 4);
    Ok(())
}

#[test]
fn execution_scope_failure_discards_successful_staged_steps() -> Result {
    let mut world = world()?;
    let mut high = ottd_save::TileRawParts::from(world.map().tiles().get(652).ok_or("tile")?);
    high.height = 5;
    world.edit_tile(652, &high.into())?;
    let mut water = ottd_save::TileRawParts::from(world.map().tiles().get(648).ok_or("tile")?);
    water.tile_type = 0x60;
    water.m5 = 0;
    world.edit_tile(648, &water.into())?;
    let before = world.saved_json()?;
    let mut command = request(650, 652, 2);
    command.mode = CommandMode::Estimate;
    assert!(execute_command(&mut world, &command)?.posted);
    command.mode = CommandMode::Post;
    assert!(matches!(
        execute_command(&mut world, &command),
        Err(ottd_sim::CommandError::Unsupported(
            "clearing non-clear terrain"
        ))
    ));
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
