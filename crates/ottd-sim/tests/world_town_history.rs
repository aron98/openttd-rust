//! Native town history arithmetic through the saved-world state loop.
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};

#[test]
fn large_supplied_history_retains_native_narrow_accumulator()
-> Result<(), Box<dyn std::error::Error>> {
    let mut world = World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/replay/clear-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let mut edits = vec![WorldEdit::Field {
        chunk: *b"CITY",
        record: 0,
        path: vec![PathElement::Field("valid_history".into())],
        value: WireValue::Unsigned((1_u64 << 61).saturating_sub(1)),
    }];
    edits.push(WorldEdit::StructList {
        chunk: *b"CITY",
        record: 0,
        path: vec![PathElement::Field("supplied".into())],
        rows: supplied_template()?,
    });
    let date = i64::from(ottd_core::CalendarDate::from_ymd(2100, 11, 31)?.raw());
    for (name, value) in [
        ("date", WireValue::Signed(date)),
        ("economy_date", WireValue::Signed(date)),
        ("date_fract", WireValue::Unsigned(73)),
        ("economy_date_fract", WireValue::Unsigned(73)),
    ] {
        edits.push(WorldEdit::Field {
            chunk: *b"DATE",
            record: 0,
            path: vec![PathElement::Field(name.into())],
            value,
        });
    }
    for index in 0..61 {
        for name in ["production", "transported"] {
            edits.push(WorldEdit::Field {
                chunk: *b"CITY",
                record: 0,
                path: vec![
                    PathElement::Field("supplied".into()),
                    PathElement::Index(0),
                    PathElement::Field("history".into()),
                    PathElement::Index(index),
                    PathElement::Field(name.into()),
                ],
                value: WireValue::Unsigned(1_000_000_000),
            });
        }
    }
    world.edit_batch(edits)?;
    ottd_sim::advance_world(&mut world, 1)?;
    let saved = world.saved_json()?;
    let mut recorded = Vec::new();
    for path in [
        "/chunks/CITY/records/0/supplied/0/history/25/production",
        "/chunks/CITY/records/0/supplied/0/history/25/transported",
        "/chunks/CITY/records/0/supplied/0/history/42/production",
        "/chunks/CITY/records/0/supplied/0/history/42/transported",
    ] {
        recorded.push(
            saved
                .pointer(path)
                .and_then(serde_json::Value::as_u64)
                .ok_or("native history field")?,
        );
    }
    assert_eq!(recorded, [4_294_967_295; 4]);
    assert_eq!(
        saved
            .pointer("/chunks/CITY/records/0/supplied/0/history/1/production")
            .and_then(serde_json::Value::as_u64),
        Some(1_000_000_000)
    );
    Ok(())
}

fn supplied_template() -> Result<Vec<ottd_save::TableRecord>, Box<dyn std::error::Error>> {
    let populated = World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/world/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let table = populated.tables().get(b"CITY").ok_or("town table")?;
    let row = table.records().get(&0).ok_or("town")?;
    let value = table
        .schema()
        .fields()
        .iter()
        .zip(row.values())
        .find_map(|(field, value)| (field.name() == "supplied").then_some(value))
        .ok_or("supplied history")?;
    let WireValue::Structs(rows) = value else {
        return Err("supplied history shape".into());
    };
    Ok(rows.clone())
}
