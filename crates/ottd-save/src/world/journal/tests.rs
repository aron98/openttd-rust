use super::*;
use crate::{Compression, Savegame, TileRawParts, world::PathElement};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn world() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../../../fixtures/world/populated-v362.sav"),
        crate::DEFAULT_MAX_BYTES,
    )?)?)
}
fn field(chunk: [u8; 4], record: u32, name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record,
        path: vec![PathElement::Field(name.into())],
        value,
    }
}
fn money() -> WorldEdit {
    field(*b"PLYR", 0, "money", WireValue::Signed(111_111))
}
fn record(world: &World, chunk: [u8; 4], id: u32) -> Result<TableRecord> {
    Ok(world
        .tables
        .get(&chunk)
        .and_then(|table| table.records().get(&id))
        .ok_or("record")?
        .clone())
}
fn common(name: &str) -> Vec<PathElement> {
    vec![
        PathElement::Field("roadveh".into()),
        PathElement::Index(0),
        PathElement::Field("common".into()),
        PathElement::Index(0),
        PathElement::Field(name.into()),
    ]
}
fn cargo(world: &World) -> Result<Vec<WorldEdit>> {
    let mut row = record(world, *b"VEHS", 21)?;
    let schema = world.tables.get(b"VEHS").ok_or("vehicles")?.schema();
    *edit::select_mut(schema, &mut row, &common("cargo.packets"))? = WireValue::Array(vec![
        WireValue::Unsigned(4),
        WireValue::Unsigned(3),
        WireValue::Unsigned(6),
    ]);
    *edit::select_mut(schema, &mut row, &common("cargo.action_counts"))? = WireValue::Array(vec![
        WireValue::Unsigned(0),
        WireValue::Unsigned(0),
        WireValue::Unsigned(28),
        WireValue::Unsigned(0),
    ]);
    Ok(vec![
        WorldEdit::ReplaceRecord {
            chunk: *b"VEHS",
            record: 21,
            value: row,
        },
        WorldEdit::InsertRecord {
            chunk: *b"CAPA",
            record: 5,
            value: record(world, *b"CAPA", 2)?,
        },
    ])
}
fn compare(base: &World, edits: Vec<WorldEdit>) -> Result {
    let mut legacy = base.clone();
    let expected = legacy.edit_batch_legacy(edits.clone());
    let mut actual = base.clone();
    let result = {
        let mut tx = actual.transaction();
        let staged = edits.into_iter().try_for_each(|edit| tx.apply(edit));
        staged.and_then(|()| tx.prepare().map(PreparedWorldTransaction::commit))
    };
    assert_eq!(
        result.is_ok(),
        expected.is_ok(),
        "journal {result:?}, legacy {expected:?}"
    );
    assert_eq!(actual.saved_json()?, legacy.saved_json()?);
    assert_eq!(
        serde_json::to_value(actual.derived())?,
        serde_json::to_value(legacy.derived())?
    );
    assert_eq!(actual.snapshot, legacy.snapshot);
    assert_eq!(actual.tables, legacy.tables);
    assert_eq!(
        actual.encode(Compression::None)?,
        legacy.encode(Compression::None)?
    );
    Ok(())
}
#[test]
fn journal_matches_legacy_coupled_edits() -> Result {
    let base = world()?;
    let engine = record(&base, *b"ENGN", 1)?;
    let mut tile = TileRawParts::from(base.map().tiles().first().ok_or("tile")?);
    tile.height = tile.height.wrapping_add(1);
    let mut cases = vec![
        vec![],
        vec![money()],
        vec![field(
            *b"PATS",
            0,
            "game_creation.landscape",
            WireValue::Unsigned(99),
        )],
        cargo(&base)?,
        vec![WorldEdit::Tile {
            index: 0,
            value: tile.into(),
        }],
        vec![
            field(*b"MAPS", 0, "dim_x", WireValue::Unsigned(128)),
            field(*b"MAPS", 0, "dim_y", WireValue::Unsigned(512)),
        ],
        vec![
            WorldEdit::RemoveRecord {
                chunk: *b"ENGN",
                record: 1,
            },
            WorldEdit::InsertRecord {
                chunk: *b"ENGN",
                record: 1,
                value: engine.clone(),
            },
            WorldEdit::ReplaceRecord {
                chunk: *b"ENGN",
                record: 1,
                value: engine,
            },
        ],
        vec![WorldEdit::InsertRecord {
            chunk: *b"CAPA",
            record: 500,
            value: record(&base, *b"CAPA", 2)?,
        }],
        vec![
            WorldEdit::InsertRecord {
                chunk: *b"CAPA",
                record: 500,
                value: record(&base, *b"CAPA", 2)?,
            },
            WorldEdit::RemoveRecord {
                chunk: *b"CAPA",
                record: 500,
            },
        ],
    ];
    let orders = base.tables.get(b"ORDL").ok_or("orders")?;
    let row = orders.records().get(&0).ok_or("orders")?;
    let values = row.values().first().ok_or("orders")?;
    let WireValue::Structs(rows) = values else {
        return Err("orders structure".into());
    };
    cases.push(vec![WorldEdit::StructList {
        chunk: *b"ORDL",
        record: 0,
        path: vec![PathElement::Field("orders".into())],
        rows: rows.clone(),
    }]);
    for edits in cases {
        compare(&base, edits)?;
    }
    Ok(())
}
#[test]
fn journal_matches_legacy_failures() -> Result {
    let base = world()?;
    let mut company = record(&base, *b"PLYR", 0)?;
    let schema = base.tables.get(b"PLYR").ok_or("companies")?.schema();
    *edit::select_mut(
        schema,
        &mut company,
        &[PathElement::Field("yearly_expenses".into())],
    )? = WireValue::Array(Vec::new());
    let mut tile = TileRawParts::from(base.map().tiles().first().ok_or("tile")?);
    tile.height = tile.height.wrapping_add(1);
    for bad in [
        field(*b"DATE", 0, "pause_mode", WireValue::Unsigned(256)),
        field(*b"DATE", 0, "id", WireValue::Bytes(vec![255])),
        field(
            *b"DATE",
            0,
            "competitors_interval_fired",
            WireValue::Signed(2),
        ),
        field(*b"MAPS", 0, "dim_x", WireValue::Unsigned(65)),
        field(*b"PATS", 0, "economy.inflation", WireValue::Signed(2)),
        field(*b"PLYR", 0, "is_ai", WireValue::Signed(1)),
        WorldEdit::ReplaceRecord {
            chunk: *b"PLYR",
            record: 0,
            value: company,
        },
        WorldEdit::RemoveRecord {
            chunk: *b"ENGN",
            record: 1,
        },
        WorldEdit::RemoveRecord {
            chunk: *b"CAPA",
            record: 2,
        },
        WorldEdit::InsertRecord {
            chunk: *b"ORDL",
            record: 64000,
            value: record(&base, *b"ORDL", 0)?,
        },
        WorldEdit::ReplaceRecord {
            chunk: *b"CAPA",
            record: 2,
            value: TableRecord::new(Vec::new()),
        },
        WorldEdit::Field {
            chunk: *b"VEHS",
            record: 21,
            path: common("next"),
            value: WireValue::Unsigned(22),
        },
    ] {
        assert!(
            base.clone().edit_batch_legacy(vec![bad.clone()]).is_err(),
            "expected rejection for {bad:?}"
        );
        compare(
            &base,
            vec![
                money(),
                WorldEdit::Tile {
                    index: 0,
                    value: tile.into(),
                },
                bad,
            ],
        )?;
    }
    Ok(())
}
#[test]
fn private_validation_guard_restores_on_unwind() -> Result {
    let mut world = world()?;
    let before = world.saved_json()?;
    let map_before = world.map().clone();
    let mut tx = world.transaction();
    tx.apply(money())?;
    let mut tile = TileRawParts::from(&tx.view().tile(0)?);
    tile.height = tile.height.wrapping_add(1);
    tx.apply(WorldEdit::Tile {
        index: 0,
        value: tile.into(),
    })?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _installed = Installed::new(tx.world, &mut tx.records, &mut tx.tiles);
        std::panic::resume_unwind(Box::new("validation unwound"));
    }));
    assert!(result.is_err());
    drop(tx);
    assert_eq!(world.saved_json()?, before);
    assert_eq!(world.map(), &map_before);
    Ok(())
}
#[test]
fn counted_container_size_includes_all_framing_and_enforces_the_limit() -> Result {
    let world = world()?;
    let bytes = world.encode(Compression::None)?.len();
    assert_eq!(wire_len(&world, crate::DEFAULT_MAX_BYTES)?, bytes);
    assert_eq!(wire_len(&world, bytes)?, bytes);
    assert!(wire_len(&world, bytes.saturating_sub(1)).is_err());
    Ok(())
}
#[test]
#[ignore = "native interoperability export: set WORLD_JOURNAL_OUTPUT"]
fn export_journal_native_witness() -> Result {
    let mut world = world()?;
    let edits = cargo(&world)?;
    let mut tx = world.transaction();
    for edit in edits {
        tx.apply(edit)?;
    }
    tx.prepare()?.commit();
    std::fs::write(
        std::env::var("WORLD_JOURNAL_OUTPUT")?,
        world.encode(Compression::Zlib)?,
    )?;
    Ok(())
}

#[test]
fn order_duration_delta_rejects_wrong_live_transition_before_publication() -> Result {
    // Given an authoritative order-list cache and unchanged candidate records.
    let mut world = world()?;
    let before = world.saved_json()?;
    let list = world
        .derived()
        .order_lists
        .first()
        .ok_or("order list")?
        .clone();
    let tx = world.transaction();
    let prepared = tx.prepare()?;
    // When a claimed wait removal is absent from the candidate.
    let result = prepared.with_order_duration_deltas(&[OrderDurationDelta {
        before: list,
        wait_removed: 1,
        timetabled_wait_removed: 0,
    }]);
    // Then it cannot be published and dropping restores everything.
    assert!(result.is_err());
    drop(result);
    assert_eq!(world.saved_json()?, before);
    Ok(())
}

#[test]
fn unreferenced_saved_order_list_keeps_native_afterload_caches_uninitialized() -> Result {
    // Given a complete orphan saved list with nonzero raw timetable payload.
    let mut world = world()?;
    let schema = world
        .tables
        .get(b"ORDL")
        .ok_or("ORDL")?
        .schema()
        .fields()
        .first()
        .and_then(crate::FieldSchema::child)
        .ok_or("order schema")?;
    let order = TableRecord::new(
        schema
            .fields()
            .iter()
            .map(|f| {
                WireValue::Unsigned(match f.name() {
                    "type" => 2,
                    "flags" => 136,
                    "refit_cargo" => 254,
                    "wait_time" => 31,
                    "travel_time" => 47,
                    "max_speed" => 65535,
                    _ => 0,
                })
            })
            .collect(),
    );
    let record = TableRecord::new(vec![WireValue::Structs(vec![order])]);
    world.edit_batch(vec![WorldEdit::InsertRecord {
        chunk: *b"ORDL",
        record: 60000,
        value: record.clone(),
    }])?;
    // When native-style structural restoration sees no shared-chain head.
    let restored = World::decode(&world.to_savegame()?)?;
    let cache = restored
        .derived()
        .order_lists
        .iter()
        .find(|l| l.id == 60000)
        .ok_or("cache")?;
    // Then saved rows remain exact while untouched constructor caches stay zero.
    assert_eq!(
        (
            cache.num_manual_orders,
            cache.total_duration,
            cache.timetable_duration
        ),
        (0, 0, 0)
    );
    assert_eq!(
        restored
            .tables
            .get(b"ORDL")
            .ok_or("ORDL")?
            .records()
            .get(&60000),
        Some(&record)
    );
    let mut tx = world.transaction();
    tx.apply(WorldEdit::Field {
        chunk: *b"ORDL",
        record: 60000,
        path: vec![
            PathElement::Field("orders".into()),
            PathElement::Index(0),
            PathElement::Field("wait_time".into()),
        ],
        value: WireValue::Unsigned(32),
    })?;
    tx.prepare()?.with_order_duration_deltas(&[])?.commit();
    assert_eq!(
        world
            .derived()
            .order_lists
            .iter()
            .find(|l| l.id == 60000)
            .ok_or("cache")?
            .total_duration,
        0
    );
    Ok(())
}
