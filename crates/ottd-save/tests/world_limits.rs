//! Corrupt native-handler sizes that are absent from table field descriptors.
#![cfg(test)]
use ottd_save::{Savegame, TableChunk, TableRecord, TableSchema, WireValue, world::World};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn world() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/world/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}

fn resize_nested(
    schema: &TableSchema,
    row: &mut TableRecord,
    path: &[&str],
    length: usize,
) -> Result<bool> {
    let (field, rest) = path.split_first().ok_or("missing path")?;
    let position = schema
        .fields()
        .iter()
        .position(|f| f.name() == *field)
        .ok_or("field")?;
    let descriptor = schema.fields().get(position).ok_or("descriptor")?;
    let WireValue::Structs(records) = row.values_mut().get_mut(position).ok_or("value")? else {
        return Err("struct list expected".into());
    };
    if rest.is_empty() {
        let Some(record) = records.first().cloned() else {
            return Ok(false);
        };
        records.resize(length, record);
        return Ok(true);
    }
    for record in records {
        if resize_nested(descriptor.child().ok_or("child")?, record, rest, length)? {
            return Ok(true);
        }
    }
    Ok(false)
}
fn resized(world: &World, chunk: [u8; 4], path: &[&str], length: usize) -> Result<Savegame> {
    let mut table = world.tables().get(&chunk).ok_or("table")?.clone();
    let schema = table.schema().clone();
    let mut changed = false;
    for row in table.records_mut().values_mut() {
        if resize_nested(&schema, row, path, length)? {
            changed = true;
            break;
        }
    }
    if !changed {
        return Err("fixture has no list to resize".into());
    }
    let mut save = world.to_savegame()?;
    save.replace_chunk(table.encode()?)?;
    Ok(save)
}
#[test]
fn rejects_excess_custom_handler_list_lengths() -> Result {
    let world = world()?;
    let cases = [
        (*b"CITY", vec!["received"], 7),
        (*b"CITY", vec!["supplied", "history"], 62),
        (*b"INDY", vec!["produced", "history"], 62),
        (*b"INDY", vec!["accepted"], 17),
        (*b"INDY", vec!["produced"], 17),
        (*b"PLYR", vec!["old_economy"], 25),
        (*b"PLYR", vec!["liveries"], 24),
        (*b"STNN", vec!["normal", "goods"], 65),
        (*b"LGRP", vec!["nodes"], 65536),
        (*b"LGRP", vec!["nodes", "edges"], 65536),
        (*b"LGRJ", vec!["linkgraph", "nodes"], 65536),
    ];
    let mut accepted = Vec::new();
    for (chunk, path, length) in cases {
        let save = resized(&world, chunk, &path, length)?;
        if World::decode(&save).is_ok() {
            accepted.push(format!(
                "{} {} {length}",
                String::from_utf8_lossy(&chunk),
                path.join("/")
            ));
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted oversized lists: {accepted:?}"
    );
    Ok(())
}
#[test]
fn rejects_industry_builder_id_at_native_limit() -> Result {
    let world = world()?;
    let mut save = world.to_savegame()?;
    let mut table = world
        .tables()
        .get(b"ITBL")
        .ok_or("industry builder")?
        .clone();
    let record = table.records().values().next().ok_or("record")?.clone();
    table.records_mut().insert(240, record);
    save.replace_chunk(table.encode()?)?;
    assert!(World::decode(&save).is_err());
    Ok(())
}
#[test]
fn rejects_native_metadata_discriminators_out_of_range() -> Result {
    let mut world = world()?;
    assert!(
        world
            .edit_field(
                *b"GLOG",
                0,
                &[
                    ottd_save::world::PathElement::Field("action".into()),
                    ottd_save::world::PathElement::Index(0),
                    ottd_save::world::PathElement::Field("ct".into())
                ],
                WireValue::Unsigned(255)
            )
            .is_err()
    );
    assert!(
        world
            .edit_field(
                *b"EIDS",
                0,
                &[ottd_save::world::PathElement::Field("type".into())],
                WireValue::Unsigned(4)
            )
            .is_err()
    );
    assert!(
        world
            .edit_field(
                *b"CITY",
                0,
                &[ottd_save::world::PathElement::Field("townnametype".into())],
                WireValue::Unsigned(0)
            )
            .is_err()
    );
    Ok(())
}

fn flat_record(fields: &[(&str, u8)], payload: &[u8]) -> Result<TableRecord> {
    let mut header = Vec::new();
    for (name, kind) in fields {
        header.push(*kind);
        header.push(u8::try_from(name.len())?);
        header.extend_from_slice(name.as_bytes());
    }
    header.push(0);
    let mut bytes = b"OTTN\x01\x6a\0\0TEST\x03".to_vec();
    bytes.push(u8::try_from(
        header.len().checked_add(1).ok_or("header length")?,
    )?);
    bytes.extend(header);
    bytes.push(u8::try_from(
        payload.len().checked_add(1).ok_or("record length")?,
    )?);
    bytes.extend_from_slice(payload);
    bytes.extend_from_slice(&[0; 5]);
    let save = Savegame::decode(&bytes, 4096)?;
    let table = TableChunk::decode(
        save.chunks().first().ok_or("table")?,
        ottd_save::TableTailPolicy::Reject,
    )?;
    Ok(table.records().get(&0).ok_or("record")?.clone())
}

#[test]
fn enforces_mapping_manager_index_boundaries() -> Result {
    let world = world()?;
    let template = flat_record(
        &[("grfid", 6), ("entity_id", 4), ("substitute_id", 4)],
        &[0; 8],
    )?;
    for (chunk, limit) in [
        (*b"IIDS", 240),
        (*b"TIDS", 512),
        (*b"HIDS", 4096),
        (*b"OBID", 64000),
        (*b"APID", 128),
        (*b"ATID", 256),
    ] {
        for (index, accepted) in [(limit - 1, true), (limit, false)] {
            let mut save = world.to_savegame()?;
            let mut table = world.tables().get(&chunk).ok_or("mapping table")?.clone();
            table.records_mut().insert(index, template.clone());
            save.replace_chunk(table.encode()?)?;
            assert_eq!(
                World::decode(&save).is_ok(),
                accepted,
                "{} index {index}",
                String::from_utf8_lossy(&chunk)
            );
        }
    }
    Ok(())
}

#[test]
fn enforces_station_spec_list_u8_length_boundary() -> Result {
    let world = world()?;
    let template = flat_record(&[("grfid", 6), ("localidx", 4)], &[0; 6])?;
    for field in ["speclist", "roadstopspeclist"] {
        for (length, accepted) in [(255, true), (256, false)] {
            let mut save = world.to_savegame()?;
            let mut table = world.tables().get(b"STNN").ok_or("station table")?.clone();
            let position = table
                .schema()
                .fields()
                .iter()
                .position(|f| f.name() == field)
                .ok_or("spec field")?;
            let row = table.records_mut().values_mut().next().ok_or("station")?;
            *row.values_mut().get_mut(position).ok_or("spec list")? =
                WireValue::Structs(vec![template.clone(); length]);
            save.replace_chunk(table.encode()?)?;
            assert_eq!(
                World::decode(&save).is_ok(),
                accepted,
                "{field} length {length}"
            );
        }
    }
    Ok(())
}

#[test]
fn accepts_native_partial_histories_without_inventing_missing_entries() -> Result {
    let world = world()?;
    for (chunk, path) in [
        (*b"CITY", vec!["received"]),
        (*b"CITY", vec!["supplied", "history"]),
        (*b"INDY", vec!["produced", "history"]),
    ] {
        for length in [0, 1] {
            World::decode(&resized(&world, chunk, &path, length)?)?;
        }
    }
    Ok(())
}
