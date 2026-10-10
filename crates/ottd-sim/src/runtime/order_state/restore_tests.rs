use super::*;
use crate::{Command, CommandMode, CommandRequest, CommandReturn};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
pub(super) fn sell(tile: u32, vehicle: u32, backup_order: bool) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command: Command::SellVehicle {
            location: tile,
            vehicle,
            sell_chain: false,
            backup_order,
            client_id: 77,
        },
    }
}
pub(super) fn build(tile: u32, client_id: u32) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command: Command::BuildVehicle {
            tile,
            engine: 116,
            cargo: 255,
            use_free_vehicles: false,
            client_id,
        },
    }
}
pub(super) fn copied_backup() -> Result<(SimulationRuntime, u32, u32, Vec<ottd_save::TableRecord>)>
{
    let (mut runtime, tile, _, first, second) = super::tests::shared_fixture()?;
    assert!(runtime.execute_command(&sell(tile, second, false))?.posted);
    let orders = view::row(runtime.world(), *b"ORDL", 60000)?
        .children("orders")?
        .1
        .to_vec();
    assert!(runtime.execute_command(&sell(tile, first, true))?.posted);
    Ok((runtime, tile, first, orders))
}
#[test]
fn purchase_restores_owned_copy_and_consumes_backup_in_one_lifetime() -> Result {
    // Given the actual supported sale-to-live-backup sequence, without reload.
    let (mut runtime, tile, freed, expected) = copied_backup()?;
    // When buying with matching local client identity.
    let receipt = runtime.execute_command(&build(tile, 77))?;
    // Then copied orders survive identity reuse and the consumed backup is freed.
    let Some(CommandReturn::Vehicle { vehicle, .. }) = receipt.returns.ok_or("return")?.result
    else {
        return Err("vehicle return".into());
    };
    assert_eq!(vehicle, freed);
    assert_eq!(runtime.order_backup_pool().items, 0);
    let list = view::consist(runtime.world(), vehicle)?.number("orders")?;
    assert_ne!(list, 0);
    let id = u32::try_from(list.checked_sub(1).ok_or("list")?)?;
    assert_eq!(
        view::row(runtime.world(), *b"ORDL", id)?
            .children("orders")?
            .1,
        expected
    );
    Ok(())
}
#[test]
fn purchase_with_unrelated_live_user_preserves_backup_and_builds_empty_orders() -> Result {
    // Given an owned backup for a different user.
    let (mut runtime, tile, _, _) = copied_backup()?;
    let before = view::table(runtime.world(), *b"BKOR")?.clone();
    let pool = runtime.order_backup_pool();
    // When purchasing for another client.
    let receipt = runtime.execute_command(&build(tile, 78))?;
    let Some(CommandReturn::Vehicle { vehicle, .. }) = receipt.returns.ok_or("return")?.result
    else {
        return Err("vehicle return".into());
    };
    // Then the unrelated row and allocator history survive.
    assert_eq!(
        view::consist(runtime.world(), vehicle)?.number("orders")?,
        0
    );
    assert_eq!(view::table(runtime.world(), *b"BKOR")?, &before);
    assert_eq!(runtime.order_backup_pool(), pool);
    Ok(())
}
#[test]
fn matching_restore_estimate_does_not_consume_backup_or_allocate_lists() -> Result {
    // Given a live matching backup whose payload execution would move.
    let (mut runtime, tile, _, _) = copied_backup()?;
    let saved = runtime.world.saved_json()?;
    let derived = runtime.world.derived().clone();
    let orders = runtime.orders.clone();
    let allocation = runtime.allocation.clone();
    let road = runtime.road.clone();
    let mut request = build(tile, 77);
    request.mode = CommandMode::Estimate;
    // When estimating, the native command must stop before Restore.
    let result = runtime.execute_command(&request)?;
    // Then successful cost has no world or metadata publication.
    assert!(result.result.ok_or("result")?.success);
    assert!(result.exec.is_none());
    assert_eq!(runtime.world.saved_json()?, saved);
    assert_eq!(runtime.world.derived(), &derived);
    assert_eq!(runtime.orders, orders);
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, road);
    Ok(())
}

#[test]
fn restore_post_zero_matches_server_user_and_preserves_primitive_zero() -> Result {
    // Given literal user0 and user1 copies created through the primitive API.
    let (mut runtime, tile, _, first, second) = super::tests::shared_fixture()?;
    assert!(runtime.execute_command(&sell(tile, second, false))?.posted);
    runtime.backup_orders(VehicleId::new(first), 0)?;
    runtime.backup_orders(VehicleId::new(first), 1)?;
    let literal = view::row(runtime.world(), *b"BKOR", 0)?.record.clone();
    // When public Post uses INVALID_CLIENT_ID, only server user1 matches.
    let receipt = runtime.execute_command(&build(tile, 0))?;
    let Some(CommandReturn::Vehicle { vehicle, .. }) = receipt.returns.ok_or("return")?.result
    else {
        return Err("vehicle return".into());
    };
    // Then literal user0 stays untouched and owned orders reach the new vehicle.
    assert_eq!(runtime.order_backup_pool().occupied, vec![0]);
    assert_eq!(view::row(runtime.world(), *b"BKOR", 0)?.record, &literal);
    assert_ne!(
        view::consist(runtime.world(), vehicle)?.number("orders")?,
        0
    );
    Ok(())
}

#[test]
fn restore_empty_owned_backup_consumes_row_without_allocating_order_list() -> Result {
    // Given a real freshly built vehicle with no orders and its copied backup.
    let (mut runtime, tile) = crate::runtime::purchase_tests::fixture::fixture()?;
    let receipt = runtime.execute_command(&build(tile, 77))?;
    let Some(CommandReturn::Vehicle { vehicle, .. }) = receipt.returns.ok_or("return")?.result
    else {
        return Err("vehicle return".into());
    };
    runtime.backup_orders(VehicleId::new(vehicle), 77)?;
    let lists = runtime.order_list_pool();
    // When matching purchase encounters an empty owned backup.
    let receipt = runtime.execute_command(&build(tile, 77))?;
    let Some(CommandReturn::Vehicle { vehicle, .. }) = receipt.returns.ok_or("return")?.result
    else {
        return Err("vehicle return".into());
    };
    // Then native successful no-list semantics still consume the backup.
    assert_eq!(runtime.order_backup_pool().items, 0);
    assert_eq!(runtime.order_list_pool(), lists);
    assert_eq!(
        view::consist(runtime.world(), vehicle)?.number("orders")?,
        0
    );
    Ok(())
}
