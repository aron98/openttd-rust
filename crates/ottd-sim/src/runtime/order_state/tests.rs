use super::*;
use crate::runtime::purchase_tests::fixture::{fixture, request};
use crate::{CommandReturn, runtime::VehicleId};
use ottd_save::{WireValue, world::WorldEdit};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn with_backup() -> Result<World> {
    let (mut runtime, tile) = fixture()?;
    let Some(CommandReturn::Vehicle { vehicle, .. }) = runtime
        .execute_command(&request(tile))?
        .returns
        .ok_or("returns")?
        .result
    else {
        return Err("vehicle".into());
    };
    runtime.backup_orders(VehicleId::new(vehicle), 42)?;
    Ok(runtime.into_world())
}
#[test]
fn explicit_load_clears_sp_server_but_retains_client_without_resetting_pool() -> Result {
    // Given a genuine saved-shaped backup row and its offline preserved bytes.
    let world = with_backup()?;
    let saved = world.saved_json()?;
    // When applying each explicit native afterload context.
    for context in [
        RuntimeSaveContext::SinglePlayer,
        RuntimeSaveContext::NetworkServer,
        RuntimeSaveContext::NetworkClient,
    ] {
        let (runtime, receipt) = SimulationRuntime::from_loaded_vanilla(world.clone(), context)?;
        let retain = context == RuntimeSaveContext::NetworkClient;
        // Then pool allocations survive individual frees and export follows role.
        assert_eq!(runtime.order_backup_pool().items, usize::from(retain));
        assert_eq!(runtime.order_backup_pool().slots, receipt.before.slots);
        assert_eq!(
            runtime.order_backup_pool().first_unused,
            receipt.before.first_unused
        );
        assert_eq!(receipt.deleted.len(), usize::from(!retain));
        assert_eq!(
            runtime
                .saved_json()?
                .pointer("/chunks/BKOR/records")
                .ok_or("records")?,
            &serde_json::json!({})
        );
    }
    assert_eq!(world.saved_json()?, saved);
    assert!(matches!(
        SimulationRuntime::restore_vanilla(world),
        Err(RuntimeError::OrderContextRequired)
    ));
    Ok(())
}
#[test]
fn live_sp_backup_omitted_on_save_but_preserved_in_memory() -> Result {
    // Given a live single-player backup created through production API.
    let (mut runtime, tile) = fixture()?;
    let Some(CommandReturn::Vehicle { vehicle, .. }) = runtime
        .execute_command(&request(tile))?
        .returns
        .ok_or("returns")?
        .result
    else {
        return Err("vehicle".into());
    };
    runtime.backup_orders(VehicleId::new(vehicle), 42)?;
    let pool = runtime.order_backup_pool();
    // When saving then loading the projected native save.
    let projected = World::decode(&runtime.to_savegame()?)?;
    // Then projection omits rows without deleting live backups.
    assert_eq!(pool.items, 1);
    assert_eq!(runtime.order_backup_pool(), pool);
    assert!(
        projected
            .tables()
            .get(b"BKOR")
            .ok_or("BKOR")?
            .records()
            .is_empty()
    );
    assert!(
        !runtime
            .world()
            .tables()
            .get(b"BKOR")
            .ok_or("BKOR")?
            .records()
            .is_empty()
    );
    Ok(())
}
fn common(id: u32, name: &str, value: WireValue) -> WorldEdit {
    use ottd_save::world::PathElement::{Field, Index};
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: id,
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
fn shared_depot_invalidation_subtracts_once_and_preserves_current_payload() -> Result {
    let (mut runtime, tile, depot, first, _second) = shared_fixture()?;
    let before = runtime
        .world()
        .derived()
        .order_lists
        .iter()
        .find(|l| l.id == 60000)
        .ok_or("list")?
        .clone();
    // When invalidating the road depot through its real lifecycle primitive.
    runtime.invalidate_depot_orders(depot, tile)?;
    // Then the shared list delta occurs once, nearest scheduled order stays, current nearest becomes dummy.
    let after = runtime
        .world()
        .derived()
        .order_lists
        .iter()
        .find(|l| l.id == 60000)
        .ok_or("list")?;
    assert_eq!(after.total_duration, before.total_duration - 7);
    assert_eq!(after.timetable_duration, before.timetable_duration - 7);
    assert_eq!(after.vehicles, before.vehicles);
    let current = crate::runtime::SavedVehicleView::new(runtime.world(), VehicleId::new(first))?;
    assert_eq!(current.common_number("current_order.type")?, 5);
    assert_eq!(current.common_number("current_order.flags")?, 0);
    assert_eq!(current.common_number("current_order.wait_time")?, 13);
    assert_eq!(current.common_number("current_order.travel_time")?, 17);
    let (schema, rows) = view::row(runtime.world(), *b"ORDL", 60000)?.children("orders")?;
    assert_eq!(
        (view::Row {
            schema,
            record: rows.first().ok_or("order")?
        })
        .number("flags")?,
        128
    );
    assert_eq!(
        (view::Row {
            schema,
            record: rows.get(1).ok_or("order")?
        })
        .number("type")?,
        2
    );
    Ok(())
}
#[test]
fn resetting_all_users_at_tile_preserves_capacity_and_reuses_hole() -> Result {
    // Given two users at one depot.
    let (mut runtime, tile) = fixture()?;
    let Some(CommandReturn::Vehicle { vehicle, .. }) = runtime
        .execute_command(&request(tile))?
        .returns
        .ok_or("return")?
        .result
    else {
        return Err("vehicle".into());
    };
    runtime.backup_orders(VehicleId::new(vehicle), 1)?;
    runtime.backup_orders(VehicleId::new(vehicle), 2)?;
    let pool = runtime.order_backup_pool();
    // When game logic resets the tile across all users.
    runtime.reset_order_backups(BackupReset::AtTile(tile))?;
    // Then objects are freed individually; next creation reuses the hole.
    assert_eq!(runtime.order_backup_pool().items, 0);
    assert_eq!(runtime.order_backup_pool().slots, pool.slots);
    runtime.backup_orders(VehicleId::new(vehicle), 3)?;
    assert_eq!(runtime.order_backup_pool().occupied, vec![0]);
    Ok(())
}
#[test]
fn backup_preserves_signed_time_bits_and_selective_flags() -> Result {
    // Given source values whose BKOR wire signedness differs from VEHS.
    let (mut runtime, tile) = fixture()?;
    let Some(CommandReturn::Vehicle { vehicle, .. }) = runtime
        .execute_command(&request(tile))?
        .returns
        .ok_or("return")?
        .result
    else {
        return Err("vehicle".into());
    };
    let mut world = runtime.into_world();
    world.edit_batch(vec![
        common(vehicle, "current_order_time", WireValue::Signed(-1)),
        common(vehicle, "lateness_counter", WireValue::Signed(-7)),
        common(vehicle, "vehicle_flags", WireValue::Unsigned(1023)),
    ])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    // When the native backup constructor copies initialized consist fields.
    runtime.backup_orders(VehicleId::new(vehicle), 42)?;
    // Then full32bit patterns survive and unrelated flags do not.
    let row = view::row(runtime.world(), *b"BKOR", 0)?;
    assert_eq!(row.number("current_order_time")?, u64::from(u32::MAX));
    assert_eq!(row.field("lateness_counter")?, &WireValue::Signed(-7));
    assert_eq!(row.number("vehicle_flags")?, 0x338);
    Ok(())
}

fn shared_fixture() -> Result<(SimulationRuntime, u32, u16, u32, u32)> {
    // Given two fresh vehicles sharing a timed and nearest-depot scheduled list.
    let (mut runtime, tile) = fixture()?;
    let mut ids = Vec::new();
    for _ in 0..2 {
        let Some(CommandReturn::Vehicle { vehicle, .. }) = runtime
            .execute_command(&request(tile))?
            .returns
            .ok_or("return")?
            .result
        else {
            return Err("vehicle".into());
        };
        ids.push(vehicle);
    }
    let first = *ids.first().ok_or("first")?;
    let second = *ids.get(1).ok_or("second")?;
    let depot = runtime
        .world()
        .map()
        .tiles()
        .get(usize::try_from(tile)?)
        .ok_or("tile")?
        .m2();
    let mut world = runtime.into_world();
    let table = world.tables().get(b"ORDL").ok_or("ORDL")?;
    let schema = table
        .schema()
        .fields()
        .first()
        .and_then(ottd_save::FieldSchema::child)
        .ok_or("order schema")?;
    let make = |flags| {
        ottd_save::TableRecord::new(
            schema
                .fields()
                .iter()
                .map(|f| {
                    WireValue::Unsigned(match f.name() {
                        "type" => 2,
                        "flags" => flags,
                        "dest" => u64::from(depot),
                        "wait_time" => 7,
                        "travel_time" => 11,
                        "refit_cargo" => 254,
                        "max_speed" => 65535,
                        _ => 0,
                    })
                })
                .collect(),
        )
    };
    let row = ottd_save::TableRecord::new(vec![WireValue::Structs(vec![make(136), make(16)])]);
    world.edit_batch(vec![
        WorldEdit::InsertRecord {
            chunk: *b"ORDL",
            record: 60000,
            value: row,
        },
        common(first, "orders", WireValue::Unsigned(60001)),
        common(second, "orders", WireValue::Unsigned(60001)),
        common(
            first,
            "next_shared",
            WireValue::Unsigned(u64::from(second) + 1),
        ),
        common(first, "current_order.type", WireValue::Unsigned(2)),
        common(first, "current_order.flags", WireValue::Unsigned(16)),
        common(
            first,
            "current_order.dest",
            WireValue::Unsigned(u64::from(depot)),
        ),
        common(first, "current_order.wait_time", WireValue::Unsigned(13)),
        common(first, "current_order.travel_time", WireValue::Unsigned(17)),
    ])?;
    let runtime = SimulationRuntime::restore_vanilla(world)?;
    Ok((runtime, tile, depot, first, second))
}
#[test]
fn airport_offsets_rotate_using_spec_dimensions_and_reject_invalid_geometry() -> Result {
    // Given vanilla international hangars from the pinned built-in specification.
    let expected = [
        vec![832, 326],
        vec![1603, 65],
        vec![838, 1344],
        vec![67, 1605],
    ];
    // When resolving all cardinal rotations around origin64 on width256.
    for (rotation, tiles) in [0, 2, 4, 6].into_iter().zip(expected) {
        // Then resolved positions follow the source dimension transform.
        assert_eq!(airport::hangar_tiles(4, rotation, 64, 256, 256)?, tiles);
    }
    assert!(airport::hangar_tiles(10, 0, 64, 256, 256).is_err());
    assert!(airport::hangar_tiles(4, 1, 64, 256, 256).is_err());
    assert!(airport::hangar_tiles(4, 0, 255, 256, 256).is_err());
    Ok(())
}
#[test]
fn clear_clone_chooses_other_shared_vehicle_before_unlink() -> Result {
    // Given a shared clone-only backup referring to the second vehicle.
    let (mut runtime, _tile, _depot, first, second) = shared_fixture()?;
    runtime.backup_orders(VehicleId::new(first), 42)?;
    assert_eq!(
        view::row(runtime.world(), *b"BKOR", 0)?.number("clone")?,
        u64::from(second) + 1
    );
    // When clearing that vehicle before unlinking its shared chain.
    runtime.clear_order_backup_vehicle(VehicleId::new(second))?;
    // Then the backup references the surviving shared head and owns no orders.
    let row = view::row(runtime.world(), *b"BKOR", 0)?;
    assert_eq!(row.number("clone")?, u64::from(first) + 1);
    assert!(row.children("orders")?.1.is_empty());
    Ok(())
}
#[test]
fn server_saves_fresh_backup_after_load_and_user_replacement_is_single_owned() -> Result {
    // Given a real runtime role selected at construction, not a mutable save flag.
    let (base, tile) = fixture()?;
    let (mut runtime, _) = SimulationRuntime::from_loaded_vanilla(
        base.into_world(),
        RuntimeSaveContext::NetworkServer,
    )?;
    let Some(CommandReturn::Vehicle { vehicle, .. }) = runtime
        .execute_command(&request(tile))?
        .returns
        .ok_or("return")?
        .result
    else {
        return Err("vehicle".into());
    };
    runtime.backup_orders(VehicleId::new(vehicle), 42)?;
    // When replacing the same user's backup.
    runtime.backup_orders(VehicleId::new(vehicle), 42)?;
    // Then one live object is saved with its exact identity and allocation remains stable.
    assert_eq!(runtime.order_backup_pool().occupied, vec![0]);
    let projected = World::decode(&runtime.to_savegame()?)?;
    assert_eq!(projected.saved_json()?, runtime.world().saved_json()?);
    Ok(())
}
#[test]
fn clear_backup_group_changes_only_matching_group() -> Result {
    // Given a backup with a group value in its canonical record.
    let mut world = with_backup()?;
    world.edit_batch(vec![WorldEdit::Field {
        chunk: *b"BKOR",
        record: 0,
        path: vec![ottd_save::world::PathElement::Field("group".into())],
        value: WireValue::Unsigned(65535),
    }])?;
    let (mut runtime, _) =
        SimulationRuntime::from_loaded_vanilla(world, RuntimeSaveContext::NetworkClient)?;
    let before = runtime.order_backup_pool();
    // When the group lifecycle clears the matching group.
    runtime.clear_order_backup_group(65535)?;
    // Then only its group becomes the native default and no allocation changes.
    assert_eq!(
        view::row(runtime.world(), *b"BKOR", 0)?.number("group")?,
        65534
    );
    assert_eq!(runtime.order_backup_pool(), before);
    Ok(())
}
#[test]
fn full_backup_pool_deletes_prior_user_before_capacity_check() -> Result {
    // Given all255 identities occupied, with one existing user to replace.
    let (mut runtime, tile) = fixture()?;
    let Some(CommandReturn::Vehicle { vehicle, .. }) = runtime
        .execute_command(&request(tile))?
        .returns
        .ok_or("returns")?
        .result
    else {
        return Err("vehicle".into());
    };
    for user in 0..255 {
        runtime.backup_orders(VehicleId::new(vehicle), user)?;
    }
    let before = runtime.order_backup_pool();
    // When replacing an existing user's backup at capacity.
    runtime.backup_orders(VehicleId::new(vehicle), 7)?;
    // Then deletion happens before capacity admission and that exact hole is reused.
    assert_eq!(runtime.order_backup_pool().items, 255);
    assert_eq!(runtime.order_backup_pool().slots, before.slots);
    assert_eq!(view::row(runtime.world(), *b"BKOR", 7)?.number("user")?, 7);
    let before = runtime.world().saved_json()?;
    runtime.backup_orders(VehicleId::new(vehicle), 999)?;
    assert_eq!(runtime.world().saved_json()?, before);
    Ok(())
}
#[test]
fn remote_clone_backup_survives_while_owned_target_backup_is_removed() -> Result {
    for shared in [false, true] {
        // Given a remote backup owning orders or retaining a shared clone.
        let (runtime, tile, depot, first, second) = shared_fixture()?;
        let mut world = runtime.into_world();
        if !shared {
            world.edit_batch(vec![
                common(first, "next_shared", WireValue::Unsigned(0)),
                common(second, "orders", WireValue::Unsigned(0)),
            ])?;
        }
        let mut runtime = SimulationRuntime::restore_vanilla(world)?;
        runtime.backup_orders(VehicleId::new(first), 42)?;
        runtime.world.edit_batch(vec![WorldEdit::Field {
            chunk: *b"BKOR",
            record: 0,
            path: vec![ottd_save::world::PathElement::Field("tile".into())],
            value: WireValue::Unsigned(u64::from(tile) + 1),
        }])?;
        // When the original depot destination is invalidated.
        runtime.invalidate_depot_orders(depot, tile)?;
        // Then only the backup with owned matching orders disappears.
        assert_eq!(runtime.order_backup_pool().items, usize::from(shared));
    }
    Ok(())
}
#[test]
fn rejected_combined_candidate_keeps_backup_pool_and_world_unchanged() -> Result {
    // Given a pending invalidation that frees live backups.
    let (mut runtime, tile, depot, first, _) = shared_fixture()?;
    runtime.backup_orders(VehicleId::new(first), 42)?;
    let before = runtime.world.saved_json()?;
    let pool = runtime.order_backup_pool();
    let plan = runtime.orders.plan_depot_invalidation(
        view::OrderReader::Committed(&runtime.world),
        &runtime.world.derived().order_lists,
        depot,
        tile,
        runtime.world.map().width(),
        runtime.world.map().height(),
    )?;
    // When a sibling command edit makes the final candidate invalid.
    let mut tx = runtime.world.transaction();
    let pending = plan.stage(&mut tx)?;
    tx.apply(common(first, "orders", WireValue::Unsigned(64001)))?;
    let result = tx.prepare();
    assert!(result.is_err());
    drop(result);
    drop(pending);
    // Then no saved or allocator state was published.
    assert_eq!(runtime.world.saved_json()?, before);
    assert_eq!(runtime.order_backup_pool(), pool);
    Ok(())
}
#[test]
fn native_invalid_tile_reset_sentinel_means_all_tiles() -> Result {
    // Given a retained live backup with an ordinary valid tile.
    for reset in [
        BackupReset::AtTile(u32::MAX),
        BackupReset::User {
            user: 42,
            tile: Some(u32::MAX),
        },
    ] {
        let (mut runtime, _) = SimulationRuntime::from_loaded_vanilla(
            with_backup()?,
            RuntimeSaveContext::NetworkClient,
        )?;
        // When the native INVALID_TILE sentinel enters either reset primitive.
        runtime.reset_order_backups(reset)?;
        // Then the matching backup is deleted regardless of its tile.
        assert_eq!(runtime.order_backup_pool().items, 0);
    }
    Ok(())
}
#[test]
fn loaded_nonzero_slot_retains_native_zero_index_and_preflights_deletion() -> Result {
    // Given two wire rows loaded by a genuine client-shaped context.
    let mut world = with_backup()?;
    let row = view::row(&world, *b"BKOR", 0)?.record.clone();
    world.edit_batch(vec![
        WorldEdit::InsertRecord {
            chunk: *b"BKOR",
            record: 1,
            value: row,
        },
        WorldEdit::Field {
            chunk: *b"BKOR",
            record: 1,
            path: vec![ottd_save::world::PathElement::Field("user".into())],
            value: WireValue::Unsigned(43),
        },
    ])?;
    world.edit_batch(vec![WorldEdit::Field {
        chunk: *b"BKOR",
        record: 1,
        path: vec![ottd_save::world::PathElement::Field("group".into())],
        value: WireValue::Unsigned(65535),
    }])?;
    for context in [
        RuntimeSaveContext::SinglePlayer,
        RuntimeSaveContext::NetworkServer,
    ] {
        assert!(matches!(
            SimulationRuntime::from_loaded_vanilla(world.clone(), context),
            Err(RuntimeError::NativeBackupIndex { slot: 1, index: 0 })
        ));
    }
    let (mut runtime, _) =
        SimulationRuntime::from_loaded_vanilla(world, RuntimeSaveContext::NetworkClient)?;
    let observed = runtime.order_state_json()?;
    assert_eq!(
        observed.pointer("/backups/1/id"),
        Some(&serde_json::json!(0))
    );
    assert_eq!(runtime.order_backup_pool().occupied, vec![0, 1]);
    // When deleting only the valid slot0, then slot1 remains a defined live object.
    runtime.reset_order_backups(BackupReset::User {
        user: 42,
        tile: None,
    })?;
    assert_eq!(runtime.order_backup_pool().occupied, vec![1]);
    let before = runtime.world.saved_json()?;
    let pool = runtime.order_backup_pool();
    // Deleting slot1 must hit the explicit native assertion boundary before publication.
    assert!(matches!(
        runtime.reset_order_backups(BackupReset::All),
        Err(RuntimeError::NativeBackupIndex { slot: 1, index: 0 })
    ));
    assert_eq!(runtime.world.saved_json()?, before);
    assert_eq!(runtime.order_backup_pool(), pool);
    let vehicle = *runtime
        .world
        .tables()
        .get(b"VEHS")
        .ok_or("vehicles")?
        .records()
        .keys()
        .next()
        .ok_or("vehicle")?;
    runtime.backup_orders(VehicleId::new(vehicle), 44)?;
    runtime.backup_orders(VehicleId::new(vehicle), 45)?;
    assert_eq!(
        runtime.order_state_json()?.pointer("/backups/2/id"),
        Some(&serde_json::json!(2))
    );
    // Read-only observation and nondeleting group edits remain supported.
    runtime.clear_order_backup_group(65535)?;
    assert_eq!(
        view::row(runtime.world(), *b"BKOR", 1)?.number("group")?,
        65534
    );
    assert_eq!(
        runtime.order_state_json()?.pointer("/backups/1/id"),
        Some(&serde_json::json!(0))
    );
    Ok(())
}
