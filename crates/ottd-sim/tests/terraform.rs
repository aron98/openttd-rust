//! Terraform command outcomes and authoritative world effects.
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World},
};
use ottd_sim::{Command, CommandMode, CommandRequest, execute_command};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn world() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/replay/clear-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
const fn request(tile: u32, slope: u8, dir_up: bool) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command: Command::TerraformLand {
            tile,
            slope,
            dir_up,
        },
    }
}
#[test]
fn raising_clears_incident_tiles_and_changes_only_requested_corner_height() -> Result {
    let mut world = world()?;
    let receipt = execute_command(&mut world, &request(650, 8, true))?;
    assert!(receipt.posted);
    for tile in [585, 586, 649, 650] {
        let tile = world.map().tiles().get(tile).ok_or("tile")?;
        assert_eq!(tile.m5(), 0);
    }
    assert_eq!(world.map().tiles().get(650).ok_or("tile")?.height(), 5);
    assert_eq!(world.map().tiles().get(649).ok_or("tile")?.height(), 4);
    let value = serde_json::to_value(receipt)?;
    assert_eq!(
        value.get("returns").and_then(|v| v.get("result")),
        Some(&serde_json::json!({"kind":"landscape","additional_money":0,"tile":650}))
    );
    Ok(())
}
#[test]
fn estimating_preserves_complete_world() -> Result {
    let mut world = world()?;
    let before = world.saved_json()?;
    let mut command = request(650, 8, true);
    command.mode = CommandMode::Estimate;
    let receipt = execute_command(&mut world, &command)?;
    assert!(receipt.posted);
    assert!(receipt.exec.is_none());
    assert_eq!(before, world.saved_json()?);
    Ok(())
}
#[test]
fn insufficient_limit_rolls_back_all_prospective_clearing() -> Result {
    let mut world = world()?;
    world.edit_field(
        *b"PLYR",
        0,
        &[PathElement::Field("terraform_limit".into())],
        WireValue::Unsigned(65535),
    )?;
    let before = world.saved_json()?;
    let receipt = execute_command(&mut world, &request(650, 8, true))?;
    assert_eq!(
        receipt.result.ok_or("result")?.error.as_deref(),
        Some("STR_ERROR_TERRAFORM_LIMIT_REACHED")
    );
    assert_eq!(before, world.saved_json()?);
    Ok(())
}
#[test]
fn high_only_mask_succeeds_without_changing_terrain() -> Result {
    let mut world = world()?;
    let before = world.map().tiles().to_vec();
    let receipt = execute_command(&mut world, &request(650, 240, true))?;
    assert!(receipt.posted);
    assert_eq!(receipt.result.ok_or("result")?.cost, 0);
    assert_eq!(before, world.map().tiles());
    Ok(())
}
#[test]
fn later_selected_corner_already_changed_by_recursion_fails_atomically() -> Result {
    let mut world = world()?;
    assert!(execute_command(&mut world, &request(650, 1, true))?.posted);
    let before = world.saved_json()?;
    let receipt = execute_command(&mut world, &request(650, 9, true))?;
    assert_eq!(
        receipt.result.ok_or("result")?.error.as_deref(),
        Some("CMD_ERROR")
    );
    assert_eq!(before, world.saved_json()?);
    Ok(())
}
#[test]
fn affordability_failure_preserves_successful_test_tuple() -> Result {
    let mut world = world()?;
    world.edit_field(
        *b"PLYR",
        0,
        &[PathElement::Field("money".into())],
        WireValue::Signed(0),
    )?;
    let before = world.saved_json()?;
    let receipt = execute_command(&mut world, &request(650, 8, true))?;
    assert!(receipt.test.as_ref().ok_or("test")?.success);
    assert!(receipt.exec.is_none());
    assert_eq!(
        receipt.result.as_ref().ok_or("result")?.error.as_deref(),
        Some("STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY")
    );
    let returned = receipt.returns.ok_or("tuple")?;
    assert_eq!(returned.test, returned.result);
    assert_eq!(
        returned.result,
        Some(ottd_sim::CommandReturn::Landscape {
            additional_money: 0,
            tile: 650
        })
    );
    assert_eq!(before, world.saved_json()?);
    Ok(())
}
#[test]
fn invalid_company_gets_default_tuple_before_body_execution() -> Result {
    let mut world = world()?;
    let mut command = request(650, 8, true);
    command.company = 15;
    let receipt = execute_command(&mut world, &command)?;
    assert!(receipt.test.is_none());
    assert_eq!(
        receipt.returns.ok_or("tuple")?.result,
        Some(ottd_sim::CommandReturn::Landscape {
            additional_money: 0,
            tile: 0
        })
    );
    Ok(())
}
#[test]
fn all_tiles_admits_void_before_native_edge_failure() -> Result {
    let mut world = world()?;
    let tile = u32::try_from(world.map().tiles().len())?
        .checked_sub(1)
        .ok_or("map")?;
    let receipt = execute_command(&mut world, &request(tile, 8, true))?;
    assert!(receipt.gate.is_none());
    assert_eq!(
        receipt.result.ok_or("result")?.error.as_deref(),
        Some("STR_ERROR_TOO_CLOSE_TO_EDGE_OF_MAP")
    );
    Ok(())
}
