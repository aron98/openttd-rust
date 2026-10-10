use super::*;
use crate::{Command, CommandMode, CommandRequest};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn sale(tile: u32, vehicle: u32, backup_order: bool, client_id: u32) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command: Command::SellVehicle {
            location: tile,
            vehicle,
            sell_chain: false,
            backup_order,
            client_id,
        },
    }
}
fn sole() -> Result<(SimulationRuntime, u32, u32)> {
    let (mut runtime, tile, _, first, second) = super::tests::shared_fixture()?;
    assert!(
        runtime
            .execute_command(&sale(tile, second, false, 77))?
            .posted
    );
    Ok((runtime, tile, first))
}
#[test]
fn backup_enabled_sale_creates_owned_copy_before_last_list_free() -> Result {
    // Given a sole owner of timed orders.
    let (mut runtime, tile, id) = sole()?;
    let expected = view::row(runtime.world(), *b"ORDL", 60000)?
        .children("orders")?
        .1
        .to_vec();
    // When the native sale requests a backup before its destructor.
    assert!(runtime.execute_command(&sale(tile, id, true, 77))?.posted);
    // Then the copy outlives the vehicle and the freed canonical list.
    let row = view::row(runtime.world(), *b"BKOR", 0)?;
    assert_eq!(row.number("user")?, 77);
    assert_eq!(row.number("clone")?, 0);
    assert_eq!(row.children("orders")?.1, expected);
    assert!(
        !view::table(runtime.world(), *b"ORDL")?
            .records()
            .contains_key(&60000)
    );
    assert!(
        !view::table(runtime.world(), *b"VEHS")?
            .records()
            .contains_key(&id)
    );
    Ok(())
}
#[test]
fn backup_enabled_post_zero_maps_to_server_but_primitive_zero_stays_literal() -> Result {
    // Given a literal primitive user0 backup, independent from command-envelope identity.
    let (mut runtime, tile, id) = sole()?;
    runtime.backup_orders(VehicleId::new(id), 0)?;
    let original = view::row(runtime.world(), *b"BKOR", 0)?.record.clone();
    // When local Post carries INVALID_CLIENT_ID.
    assert!(runtime.execute_command(&sale(tile, id, true, 0))?.posted);
    // Then server user1 is created without replacing literal primitive user0.
    assert_eq!(view::row(runtime.world(), *b"BKOR", 0)?.record, &original);
    assert_eq!(view::row(runtime.world(), *b"BKOR", 1)?.number("user")?, 1);
    assert_eq!(runtime.order_backup_pool().occupied, vec![0, 1]);
    Ok(())
}
#[test]
fn backup_enabled_estimate_keeps_existing_user_backup_and_every_allocator() -> Result {
    // Given an existing same-user backup that execution would replace.
    let (mut runtime, tile, id) = sole()?;
    runtime.backup_orders(VehicleId::new(id), 77)?;
    let saved = runtime.world.saved_json()?;
    let derived = runtime.world.derived().clone();
    let orders = runtime.orders.clone();
    let allocation = runtime.allocation.clone();
    let road = runtime.road.clone();
    let depot = runtime.depot.clone();
    let mut request = sale(tile, id, true, 77);
    request.mode = CommandMode::Estimate;
    // When only estimating the backup-enabled sale.
    let receipt = runtime.execute_command(&request)?;
    // Then the native successful estimate performs no lifecycle work.
    assert!(receipt.result.ok_or("result")?.success);
    assert!(receipt.exec.is_none());
    assert_eq!(runtime.world.saved_json()?, saved);
    assert_eq!(runtime.world.derived(), &derived);
    assert_eq!(runtime.orders, orders);
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, road);
    assert_eq!(runtime.depot, depot);
    Ok(())
}
#[test]
fn backup_enabled_same_slot_replacement_survives_candidate_clear_vehicle() -> Result {
    // Given a backup cloning the now-sole survivor.
    let (mut runtime, tile, _, first, second) = super::tests::shared_fixture()?;
    runtime.backup_orders(VehicleId::new(first), 77)?;
    assert!(
        runtime
            .execute_command(&sale(tile, first, false, 77))?
            .posted
    );
    assert_eq!(
        view::row(runtime.world(), *b"BKOR", 0)?.number("clone")?,
        u64::from(second) + 1
    );
    let expected = view::row(runtime.world(), *b"ORDL", 60000)?
        .children("orders")?
        .1
        .to_vec();
    // When Backup reuses the old clone's slot for a copy before ClearVehicle.
    assert!(
        runtime
            .execute_command(&sale(tile, second, true, 77))?
            .posted
    );
    // Then cleanup sees the new copy and cannot delete it using the old clone.
    let row = view::row(runtime.world(), *b"BKOR", 0)?;
    assert_eq!(row.number("clone")?, 0);
    assert_eq!(row.children("orders")?.1, expected);
    assert_eq!(runtime.order_backup_pool().occupied, vec![0]);
    Ok(())
}
fn capacity_sale(user: u32) -> Result<(SimulationRuntime, Vec<ottd_save::TableRecord>)> {
    let (mut runtime, tile, _, first, second) = super::tests::shared_fixture()?;
    for user in 1..=255 {
        runtime.backup_orders(VehicleId::new(first), user)?;
    }
    assert!(
        runtime
            .execute_command(&sale(tile, first, false, 77))?
            .posted
    );
    assert_eq!(runtime.order_backup_pool().items, 255);
    let expected = view::row(runtime.world(), *b"ORDL", 60000)?
        .children("orders")?
        .1
        .to_vec();
    assert!(
        runtime
            .execute_command(&sale(tile, second, true, user))?
            .posted
    );
    Ok((runtime, expected))
}
#[test]
fn backup_enabled_full_pool_checks_capacity_before_clear_vehicle_frees_clones() -> Result {
    // Given 255 clone backups, when selling with a new user, Backup precedes cleanup.
    let (runtime, _) = capacity_sale(256)?;
    // Then no new copy is created even though subsequent cleanup frees all rows.
    assert_eq!(runtime.order_backup_pool().items, 0);
    assert!(view::table(runtime.world(), *b"BKOR")?.records().is_empty());
    Ok(())
}
#[test]
fn backup_enabled_full_pool_replaces_user_then_preserves_reused_copy_slot() -> Result {
    // Given a full pool, when replacing existing user128 before clearing old clones.
    let (runtime, expected) = capacity_sale(128)?;
    // Then just the new copied backup remains at the freed user's physical slot.
    assert_eq!(runtime.order_backup_pool().occupied, vec![127]);
    let row = view::row(runtime.world(), *b"BKOR", 127)?;
    assert_eq!(row.number("user")?, 128);
    assert_eq!(row.number("clone")?, 0);
    assert_eq!(row.children("orders")?.1, expected);
    Ok(())
}
fn field(vehicle: u32, name: &str, value: ottd_save::WireValue) -> WorldEdit {
    use ottd_save::world::PathElement::{Field, Index};
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: vehicle,
        path: vec![
            Field("roadveh".into()),
            Index(0),
            Field("common".into()),
            Index(0),
            Field(name.into()),
        ],
        value,
    }
}
#[test]
fn backup_enabled_failed_candidate_rolls_back_replacement_detach_and_refund() -> Result {
    // Given an old same-user clone backup that staging replaces before cleanup.
    let (mut runtime, tile, _, first, second) = super::tests::shared_fixture()?;
    runtime.backup_orders(VehicleId::new(first), 77)?;
    assert!(
        runtime
            .execute_command(&sale(tile, first, false, 77))?
            .posted
    );
    let saved = runtime.world.saved_json()?;
    let derived = runtime.world.derived().clone();
    let orders = runtime.orders.clone();
    let allocation = runtime.allocation.clone();
    let road = runtime.road.clone();
    let depot = runtime.depot.clone();
    // When the final candidate leaves the sold vehicle referencing a freed list.
    let result = super::super::RoadVehicleContext {
        orders: &mut runtime.orders,
        serializer_cargo_paid_for: runtime.serializer_cargo_paid_for,
        content: &runtime.content,
        allocation: &mut runtime.allocation,
        road: &mut runtime.road,
    }
    .publish_sale(
        &mut runtime.world,
        vec![crate::world_access::field_edit(
            *b"PLYR",
            0,
            "money",
            ottd_save::WireValue::Signed(123),
        )],
        VehicleId::new(second),
        Some(77),
    );
    // Then none of replacement, metadata allocation, detach, or refund publishes.
    assert!(result.is_err());
    assert_eq!(runtime.world.saved_json()?, saved);
    assert_eq!(runtime.world.derived(), &derived);
    assert_eq!(runtime.orders, orders);
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, road);
    assert_eq!(runtime.depot, depot);
    Ok(())
}
#[test]
fn backup_enabled_poor_and_invalid_owner_fail_before_replacing_user_backup() -> Result {
    for poor in [false, true] {
        // Given a valid stopped vehicle with a same-user backup and discriminating cost.
        let (runtime, tile, id) = sole()?;
        let mut world = runtime.into_world();
        world.edit_batch(vec![
            field(id, "value", ottd_save::WireValue::Signed(-9)),
            crate::world_access::field_edit(*b"PLYR", 0, "money", ottd_save::WireValue::Signed(0)),
        ])?;
        let mut runtime = SimulationRuntime::restore_vanilla(world)?;
        runtime.backup_orders(VehicleId::new(id), 77)?;
        let saved = runtime.world.saved_json()?;
        let derived = runtime.world.derived().clone();
        let orders = runtime.orders.clone();
        let allocation = runtime.allocation.clone();
        let road = runtime.road.clone();
        let mut request = sale(tile, id, true, 77);
        if !poor {
            request.company = 1;
        }
        // When affordability or the native ownership gate rejects execution.
        let receipt = runtime.execute_command(&request)?;
        // Then no prior backup or allocator can change.
        assert_eq!(
            receipt.result.ok_or("result")?.error.as_deref(),
            Some(if poor {
                "STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY"
            } else {
                "STR_ERROR_OWNED_BY"
            })
        );
        assert!(receipt.exec.is_none());
        assert_eq!(runtime.world.saved_json()?, saved);
        assert_eq!(runtime.world.derived(), &derived);
        assert_eq!(runtime.orders, orders);
        assert_eq!(runtime.allocation, allocation);
        assert_eq!(runtime.road, road);
    }
    Ok(())
}
#[test]
fn backup_enabled_non_sp_rejects_even_when_backup_pool_empty() -> Result {
    for context in [
        RuntimeSaveContext::NetworkClient,
        RuntimeSaveContext::NetworkServer,
    ] {
        // Given a real typed runtime context with no loaded or live backups.
        let (runtime, tile, id) = sole()?;
        let (mut runtime, _) =
            SimulationRuntime::from_loaded_vanilla(runtime.into_world(), context)?;
        let saved = runtime.world.saved_json()?;
        let orders = runtime.orders.clone();
        let allocation = runtime.allocation.clone();
        // When the newly admitted command is requested outside SP.
        let result = runtime.execute_command(&sale(tile, id, true, 77));
        // Then the explicit domain boundary does not alter existing false-mode behavior.
        assert!(matches!(
            result,
            Err(crate::CommandError::Unsupported(
                "sale live backup host context"
            ))
        ));
        assert_eq!(runtime.world.saved_json()?, saved);
        assert_eq!(runtime.orders, orders);
        assert_eq!(runtime.allocation, allocation);
        assert!(runtime.execute_command(&sale(tile, id, false, 77))?.posted);
    }
    Ok(())
}
#[test]
fn backup_enabled_preserves_renewal_guard_for_post_estimate_empty_live_backups() -> Result {
    for backup in [false, true] {
        for mode in [CommandMode::Post, CommandMode::Estimate] {
            super::backup_sale_tests::renewal_guard(backup, mode, true)?;
        }
    }
    Ok(())
}
