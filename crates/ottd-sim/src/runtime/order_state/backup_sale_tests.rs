use super::*;
use crate::{Command, CommandMode, CommandRequest};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn sale(tile: u32, vehicle: u32) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command: Command::SellVehicle {
            location: tile,
            vehicle,
            sell_chain: false,
            backup_order: false,
            client_id: 77,
        },
    }
}
#[test]
fn live_backup_sale_redirects_clone_before_detaching_shared_member() -> Result {
    let (mut runtime, tile, _, first, second) = super::tests::shared_fixture()?;
    runtime.backup_orders(VehicleId::new(first), 77)?;
    let pool = runtime.order_backup_pool();
    assert!(runtime.execute_command(&sale(tile, second))?.posted);
    assert_eq!(runtime.order_backup_pool(), pool);
    assert_eq!(
        view::row(runtime.world(), *b"BKOR", 0)?.number("clone")?,
        u64::from(first) + 1
    );
    assert!(runtime.execute_command(&sale(tile, first))?.posted);
    let after = runtime.order_backup_pool();
    assert_eq!(after.items, 0);
    assert_eq!(after.slots, pool.slots);
    assert_eq!(after.first_unused, pool.first_unused);
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
fn copied_backups_survive_sale_and_sp_projection_without_mutation() -> Result {
    let (runtime, tile, _, first, second) = super::tests::shared_fixture()?;
    let mut world = runtime.into_world();
    world.edit_batch(vec![
        field(first, "next_shared", ottd_save::WireValue::Unsigned(0)),
        field(second, "orders", ottd_save::WireValue::Unsigned(0)),
    ])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    for user in [100, 101] {
        runtime.backup_orders(VehicleId::new(first), user)?;
    }
    let backups = runtime.world().tables().get(b"BKOR").ok_or("BKOR")?.clone();
    let pool = runtime.order_backup_pool();
    assert!(runtime.execute_command(&sale(tile, first))?.posted);
    assert_eq!(runtime.world().tables().get(b"BKOR"), Some(&backups));
    assert_eq!(runtime.order_backup_pool(), pool);
    let saved = World::decode(&runtime.to_savegame()?)?;
    assert!(
        saved
            .tables()
            .get(b"BKOR")
            .ok_or("BKOR")?
            .records()
            .is_empty()
    );
    assert_eq!(runtime.world().tables().get(b"BKOR"), Some(&backups));
    assert_eq!(runtime.order_backup_pool(), pool);
    Ok(())
}
#[test]
fn live_backup_host_boundary_preserves_native_gates_and_empty_backup_behavior() -> Result {
    for context in [
        RuntimeSaveContext::NetworkClient,
        RuntimeSaveContext::NetworkServer,
    ] {
        let (runtime, tile, _, first, second) = super::tests::shared_fixture()?;
        let (mut runtime, _) =
            SimulationRuntime::from_loaded_vanilla(runtime.into_world(), context)?;
        runtime.backup_orders(VehicleId::new(first), 77)?;
        let before = runtime.world.saved_json()?;
        let orders = runtime.orders.clone();
        let mut wrong = sale(tile, second);
        wrong.company = 1;
        assert_eq!(
            runtime
                .execute_command(&wrong)?
                .result
                .ok_or("result")?
                .error
                .as_deref(),
            Some("STR_ERROR_OWNED_BY")
        );
        assert!(matches!(
            runtime.execute_command(&sale(tile, second)),
            Err(crate::CommandError::Unsupported(
                "sale live backup host context"
            ))
        ));
        assert_eq!(runtime.world.saved_json()?, before);
        assert_eq!(runtime.orders, orders);
        runtime.reset_order_backups(BackupReset::All)?;
        assert!(runtime.execute_command(&sale(tile, second))?.posted);
    }
    Ok(())
}
#[test]
fn failed_sale_candidate_rolls_back_backup_retarget_and_all_allocators() -> Result {
    let (mut runtime, _, _, first, second) = super::tests::shared_fixture()?;
    runtime.backup_orders(VehicleId::new(first), 77)?;
    let saved = runtime.world.saved_json()?;
    let derived = runtime.world.derived().clone();
    let orders = runtime.orders.clone();
    let allocation = runtime.allocation.clone();
    let road = runtime.road.clone();
    let result = super::super::RoadVehicleContext {
        orders: &mut runtime.orders,
        serializer_cargo_paid_for: runtime.serializer_cargo_paid_for,
        content: &runtime.content,
        allocation: &mut runtime.allocation,
        road: &mut runtime.road,
    }
    .publish_sale(
        &mut runtime.world,
        vec![
            WorldEdit::RemoveRecord {
                chunk: *b"VEHS",
                record: second,
            },
            field(
                first,
                "next_shared",
                ottd_save::WireValue::Unsigned(u64::from(second) + 1),
            ),
        ],
        VehicleId::new(second),
    );
    assert!(result.is_err());
    assert_eq!(runtime.world.saved_json()?, saved);
    assert_eq!(runtime.world.derived(), &derived);
    assert_eq!(runtime.orders, orders);
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, road);
    Ok(())
}
#[test]
fn composed_sale_preflights_native_slot_index_deletion_boundary() -> Result {
    let (mut runtime, _, _, first, second) = super::tests::shared_fixture()?;
    for user in [100, 101] {
        runtime.backup_orders(VehicleId::new(first), user)?;
    }
    let mut world = runtime.into_world();
    world.edit_batch(vec![
        field(first, "next_shared", ottd_save::WireValue::Unsigned(0)),
        field(second, "orders", ottd_save::WireValue::Unsigned(0)),
    ])?;
    let (mut runtime, _) =
        SimulationRuntime::from_loaded_vanilla(world, RuntimeSaveContext::NetworkClient)?;
    let saved = runtime.world.saved_json()?;
    let derived = runtime.world.derived().clone();
    let orders = runtime.orders.clone();
    let allocation = runtime.allocation.clone();
    let road = runtime.road.clone();
    // Internal transaction control; this does not admit public network command dispatch.
    let result = super::super::RoadVehicleContext {
        orders: &mut runtime.orders,
        serializer_cargo_paid_for: runtime.serializer_cargo_paid_for,
        content: &runtime.content,
        allocation: &mut runtime.allocation,
        road: &mut runtime.road,
    }
    .publish_sale(
        &mut runtime.world,
        vec![WorldEdit::RemoveRecord {
            chunk: *b"VEHS",
            record: second,
        }],
        VehicleId::new(second),
    );
    assert!(matches!(
        result,
        Err(crate::CommandError::Runtime(
            RuntimeError::NativeBackupIndex { slot: 1, index: 0 }
        ))
    ));
    assert_eq!(runtime.world.saved_json()?, saved);
    assert_eq!(runtime.world.derived(), &derived);
    assert_eq!(runtime.orders, orders);
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, road);
    Ok(())
}

fn renewal_guard(with_backup: bool, mode: CommandMode) -> Result {
    use ottd_save::{TableRecord, WireValue, world::PathElement};
    let (runtime, tile, _, first, second) = super::tests::shared_fixture()?;
    let mut world = runtime.into_world();
    let schema = world.tables().get(b"ERNW").ok_or("ERNW")?.schema();
    assert_eq!(
        schema
            .fields()
            .iter()
            .map(ottd_save::FieldSchema::name)
            .collect::<Vec<_>>(),
        ["from", "to", "next", "group_id", "replace_when_old"]
    );
    world.edit_batch(vec![
        WorldEdit::InsertRecord {
            chunk: *b"ERNW",
            record: 0,
            value: TableRecord::new(vec![
                WireValue::Unsigned(116),
                WireValue::Unsigned(117),
                WireValue::Unsigned(0),
                WireValue::Unsigned(65534),
                WireValue::Signed(0),
            ]),
        },
        WorldEdit::Field {
            chunk: *b"PLYR",
            record: 0,
            path: vec![
                PathElement::Field("settings".into()),
                PathElement::Index(0),
                PathElement::Field("engine_renew_list".into()),
            ],
            value: WireValue::Unsigned(1),
        },
    ])?;
    let canonical = world.saved_json()?;
    let decoded = World::decode(&world.to_savegame()?)?;
    assert_eq!(decoded.saved_json()?, canonical);
    let mut runtime = SimulationRuntime::restore_vanilla(decoded)?;
    if with_backup {
        runtime.backup_orders(VehicleId::new(first), 77)?;
    }
    assert_eq!(
        view::row(runtime.world(), *b"ERNW", 0)?.number("from")?,
        116
    );
    assert_eq!(
        {
            let (schema, rows) = view::row(runtime.world(), *b"PLYR", 0)?.children("settings")?;
            view::Row {
                schema,
                record: rows.first().ok_or("settings")?,
            }
            .number("engine_renew_list")?
        },
        1
    );
    assert_eq!(runtime.order_backup_pool().items, usize::from(with_backup));
    let saved = runtime.world.saved_json()?;
    let derived = runtime.world.derived().clone();
    let orders = runtime.orders.clone();
    let allocation = runtime.allocation.clone();
    let road = runtime.road.clone();
    let depot = runtime.depot.clone();
    let serializer = runtime.serializer_cargo_paid_for;
    let groups = runtime.road_group_counts()?;
    let mut command = sale(tile, second);
    command.mode = mode;
    let result = runtime.execute_command(&command);
    assert!(
        matches!(
            result,
            Err(crate::CommandError::Unsupported(
                "sale group profit or renewal lifecycle"
            ))
        ),
        "{result:?}"
    );
    assert_eq!(runtime.world.saved_json()?, saved);
    assert_eq!(runtime.world.derived(), &derived);
    assert_eq!(runtime.orders, orders);
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, road);
    assert_eq!(runtime.depot, depot);
    assert_eq!(runtime.serializer_cargo_paid_for, serializer);
    assert_eq!(runtime.road_group_counts()?, groups);
    Ok(())
}
#[test]
fn renewal_guard_post_empty_backups() -> Result {
    renewal_guard(false, CommandMode::Post)
}
#[test]
fn renewal_guard_estimate_empty_backups() -> Result {
    renewal_guard(false, CommandMode::Estimate)
}
#[test]
fn renewal_guard_post_live_backups() -> Result {
    renewal_guard(true, CommandMode::Post)
}
#[test]
fn renewal_guard_estimate_live_backups() -> Result {
    renewal_guard(true, CommandMode::Estimate)
}
