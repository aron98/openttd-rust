//! Explicit pool edits preserve final-world validation and rollback.
use ottd_save::{
    Compression, Savegame, TableRecord, TableSchema, WireValue,
    world::{PathElement, World, WorldEdit},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn world() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/world/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
fn record(world: &World, chunk: [u8; 4], id: u32) -> Result<TableRecord> {
    Ok(world
        .tables()
        .get(&chunk)
        .ok_or("table")?
        .records()
        .get(&id)
        .ok_or("record")?
        .clone())
}
fn set(row: &mut TableRecord, schema: &TableSchema, path: &[&str], value: WireValue) -> Result {
    let (name, rest) = path.split_first().ok_or("path")?;
    let index = schema
        .fields()
        .iter()
        .position(|field| field.name() == *name)
        .ok_or("field")?;
    let target = row.values_mut().get_mut(index).ok_or("value")?;
    if rest.is_empty() {
        *target = value;
        return Ok(());
    }
    let WireValue::Structs(rows) = target else {
        return Err("structure".into());
    };
    set(
        rows.first_mut().ok_or("child")?,
        schema
            .fields()
            .get(index)
            .and_then(|field| field.child())
            .ok_or("schema")?,
        rest,
        value,
    )
}
fn changed(
    world: &World,
    chunk: [u8; 4],
    id: u32,
    changes: &[(&[&str], WireValue)],
) -> Result<TableRecord> {
    let mut row = record(world, chunk, id)?;
    let schema = world.tables().get(&chunk).ok_or("table")?.schema();
    for (path, value) in changes {
        set(&mut row, schema, path, value.clone())?;
    }
    Ok(row)
}
const fn replace_vehicle(value: TableRecord) -> WorldEdit {
    WorldEdit::ReplaceRecord {
        chunk: *b"VEHS",
        record: 21,
        value,
    }
}
fn cargo_packet() -> TableRecord {
    TableRecord::new(vec![
        WireValue::Unsigned(7),
        WireValue::Unsigned(41825),
        WireValue::Unsigned(41825),
        WireValue::Unsigned(4),
        WireValue::Unsigned(2),
        WireValue::Signed(11),
        WireValue::Unsigned(1),
        WireValue::Unsigned(15),
        WireValue::Signed(97),
        WireValue::Signed(163),
    ])
}
fn cargo_edits(world: &World) -> Result<Vec<WorldEdit>> {
    let owner = changed(
        world,
        *b"VEHS",
        21,
        &[
            (
                &["roadveh", "common", "cargo.packets"],
                WireValue::Array(vec![
                    WireValue::Unsigned(4),
                    WireValue::Unsigned(3),
                    WireValue::Unsigned(6),
                ]),
            ),
            (
                &["roadveh", "common", "cargo.action_counts"],
                WireValue::Array(vec![
                    WireValue::Unsigned(0),
                    WireValue::Unsigned(0),
                    WireValue::Unsigned(28),
                    WireValue::Unsigned(0),
                ]),
            ),
        ],
    )?;
    Ok(vec![
        replace_vehicle(owner),
        WorldEdit::InsertRecord {
            chunk: *b"CAPA",
            record: 5,
            value: cargo_packet(),
        },
    ])
}
fn assert_roundtrip(world: &World) -> Result {
    let reloaded = World::decode(&world.to_savegame()?)?;
    assert_eq!(world.saved_json()?, reloaded.saved_json()?);
    assert_eq!(
        serde_json::to_value(world.derived())?,
        serde_json::to_value(reloaded.derived())?
    );
    Ok(())
}
#[test]
fn cargo_create_and_delete_validate_only_final_coupled_state() -> Result {
    let mut world = world()?;
    let before = world.saved_json()?;
    let before_derived = serde_json::to_value(world.derived())?;
    let owner = record(&world, *b"VEHS", 21)?;
    world.edit_batch(cargo_edits(&world)?)?;
    let cargo = world
        .derived()
        .cargo_lists
        .iter()
        .find(|cargo| cargo.owner.id == 21 && cargo.cargo_type.is_none())
        .ok_or("cargo")?;
    assert_eq!(
        (&cargo.packets, cargo.count, cargo.periods_in_transit),
        (&vec![3, 2, 5], 28, 76)
    );
    assert_eq!(cargo.feeder_share, 11);
    assert_roundtrip(&world)?;
    world.edit_batch(vec![
        WorldEdit::RemoveRecord {
            chunk: *b"CAPA",
            record: 5,
        },
        replace_vehicle(owner),
    ])?;
    assert_eq!(world.saved_json()?, before);
    assert_eq!(serde_json::to_value(world.derived())?, before_derived);
    Ok(())
}
#[test]
fn linked_vehicle_and_order_records_can_be_created_and_removed_atomically() -> Result {
    let mut world = world()?;
    let before = world.saved_json()?;
    let vehicle = changed(
        &world,
        *b"VEHS",
        12,
        &[
            (&["roadveh", "common", "orders"], WireValue::Unsigned(10)),
            (
                &["roadveh", "common", "next_shared"],
                WireValue::Unsigned(102),
            ),
        ],
    )?;
    world.edit_batch(vec![
        WorldEdit::InsertRecord {
            chunk: *b"VEHS",
            record: 100,
            value: vehicle,
        },
        WorldEdit::InsertRecord {
            chunk: *b"VEHS",
            record: 101,
            value: changed(
                &world,
                *b"VEHS",
                12,
                &[
                    (&["roadveh", "common", "orders"], WireValue::Unsigned(10)),
                    (
                        &["roadveh", "common", "next_shared"],
                        WireValue::Unsigned(0),
                    ),
                ],
            )?,
        },
        WorldEdit::InsertRecord {
            chunk: *b"ORDL",
            record: 9,
            value: record(&world, *b"ORDL", 0)?,
        },
    ])?;
    let list = world
        .derived()
        .order_lists
        .iter()
        .find(|list| list.id == 9)
        .ok_or("order list")?;
    assert_eq!(
        (&list.vehicles, list.first_shared),
        (&vec![100, 101], Some(100))
    );
    let vehicle = world
        .derived()
        .vehicles
        .iter()
        .find(|vehicle| vehicle.id == 100)
        .ok_or("vehicle")?;
    assert_eq!((vehicle.first, vehicle.previous), (100, None));
    let shared = world
        .derived()
        .vehicles
        .iter()
        .find(|vehicle| vehicle.id == 101)
        .ok_or("shared vehicle")?;
    assert_eq!(shared.previous_shared, Some(100));
    assert_roundtrip(&world)?;
    world.edit_batch(vec![
        WorldEdit::RemoveRecord {
            chunk: *b"ORDL",
            record: 9,
        },
        WorldEdit::RemoveRecord {
            chunk: *b"VEHS",
            record: 100,
        },
        WorldEdit::RemoveRecord {
            chunk: *b"VEHS",
            record: 101,
        },
    ])?;
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
#[test]
fn invalid_record_operations_roll_back_saved_and_derived_state() -> Result {
    let world = world()?;
    let packet = record(&world, *b"CAPA", 2)?;
    let mut failures = vec![
        WorldEdit::InsertRecord {
            chunk: *b"CAPA",
            record: 2,
            value: packet.clone(),
        },
        WorldEdit::RemoveRecord {
            chunk: *b"CAPA",
            record: 5,
        },
        WorldEdit::ReplaceRecord {
            chunk: *b"CAPA",
            record: 5,
            value: packet.clone(),
        },
        WorldEdit::InsertRecord {
            chunk: *b"CAPA",
            record: u32::MAX,
            value: packet.clone(),
        },
        WorldEdit::InsertRecord {
            chunk: *b"ORDL",
            record: 64000,
            value: record(&world, *b"ORDL", 0)?,
        },
        WorldEdit::ReplaceRecord {
            chunk: *b"CAPA",
            record: 2,
            value: TableRecord::new(Vec::new()),
        },
        WorldEdit::RemoveRecord {
            chunk: *b"CAPA",
            record: 2,
        },
        WorldEdit::ReplaceRecord {
            chunk: *b"CAPA",
            record: 2,
            value: record(&world, *b"ORDL", 0)?,
        },
        WorldEdit::ReplaceRecord {
            chunk: *b"CAPA",
            record: 2,
            value: changed(
                &world,
                *b"CAPA",
                2,
                &[(&["count"], WireValue::Unsigned(65536))],
            )?,
        },
        WorldEdit::ReplaceRecord {
            chunk: *b"CAPA",
            record: 2,
            value: changed(
                &world,
                *b"CAPA",
                2,
                &[(&["count"], WireValue::Bytes(vec![1]))],
            )?,
        },
        replace_vehicle(changed(
            &world,
            *b"VEHS",
            21,
            &[(&["roadveh", "common", "orders"], WireValue::Unsigned(999))],
        )?),
        replace_vehicle(changed(
            &world,
            *b"VEHS",
            21,
            &[(
                &["roadveh", "common", "cargo.packets"],
                WireValue::Array(vec![WireValue::Unsigned(4), WireValue::Unsigned(4)]),
            )],
        )?),
        WorldEdit::RemoveRecord {
            chunk: *b"ORDL",
            record: 2,
        },
    ];
    for chunk in [*b"DATE", *b"MAPT", *b"NGRF", *b"AIPL", *b"????"] {
        failures.extend([
            WorldEdit::InsertRecord {
                chunk,
                record: 99,
                value: packet.clone(),
            },
            WorldEdit::ReplaceRecord {
                chunk,
                record: 0,
                value: packet.clone(),
            },
            WorldEdit::RemoveRecord { chunk, record: 0 },
        ]);
    }
    assert_rollback(world, failures)
}
fn assert_rollback(mut world: World, failures: Vec<WorldEdit>) -> Result {
    let before = world.saved_json()?;
    let before_derived = serde_json::to_value(world.derived())?;
    for failure in failures {
        let label = format!("{failure:?}");
        let result = world.edit_batch(vec![
            WorldEdit::Field {
                chunk: *b"PLYR",
                record: 0,
                path: vec![PathElement::Field("money".into())],
                value: WireValue::Signed(999_999),
            },
            failure,
        ]);
        assert!(result.is_err(), "accepted {label}");
        assert_eq!(world.saved_json()?, before, "saved rollback {label}");
        assert_eq!(
            serde_json::to_value(world.derived())?,
            before_derived,
            "derived rollback {label}"
        );
    }
    Ok(())
}
#[test]
#[ignore = "writes an API-created native save to WORLD_LIFECYCLE_OUTPUT"]
fn export_lifecycle_native_witness() -> Result {
    let output = std::env::var("WORLD_LIFECYCLE_OUTPUT")?;
    let mut world = world()?;
    world.edit_batch(cargo_edits(&world)?)?;
    std::fs::write(output, world.to_savegame()?.encode(Compression::Zlib)?)?;
    Ok(())
}

#[test]
fn unowned_packet_records_preserve_existing_structural_boundary() -> Result {
    let mut world = world()?;
    let before = world.saved_json()?;
    let packet = cargo_packet();
    assert!(packet.tail().is_empty());
    world.edit_batch(vec![WorldEdit::InsertRecord {
        chunk: *b"CAPA",
        record: 5,
        value: packet,
    }])?;
    assert_roundtrip(&world)?;
    world.edit_batch(vec![WorldEdit::RemoveRecord {
        chunk: *b"CAPA",
        record: 5,
    }])?;
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
