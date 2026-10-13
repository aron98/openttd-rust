use super::*;
pub(in crate::runtime::order_state) fn renewal_guard(
    with_backup: bool,
    mode: CommandMode,
    backup: bool,
) -> Result {
    use ottd_save::{TableRecord, WireValue, world::PathElement};
    let (runtime, tile, _, first, second) = super::super::tests::shared_fixture()?;
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
    if let Command::SellVehicle { backup_order, .. } = &mut command.command {
        *backup_order = backup;
    }
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
