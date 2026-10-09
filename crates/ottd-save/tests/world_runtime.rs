//! Runtime mutations retain complete-world validation.
use ottd_save::{
    Savegame, TileRawParts, TileState, WireValue,
    world::{PathElement, World, WorldEdit},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn world() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/world/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
#[test]
fn raw_parts_preserve_every_tile_bit() {
    let parts = TileRawParts {
        tile_type: 0x12,
        height: 3,
        m1: 4,
        m2: 0x5678,
        m3: 9,
        m4: 10,
        m5: 11,
        m6: 12,
        m7: 13,
        m8: 0xabcd,
    };
    let tile = TileState::from(parts);
    assert_eq!(TileRawParts::from(&tile), parts);
}
#[test]
fn history_list_replacement_retains_validation_and_roundtrip() -> Result {
    let mut world = world()?;
    let table = world.tables().get(b"PLYR").ok_or("company table")?;
    let row = table.records().get(&0).ok_or("company")?;
    let value = table
        .schema()
        .fields()
        .iter()
        .zip(row.values())
        .find(|(f, _)| f.name() == "old_economy")
        .ok_or("history")?
        .1;
    let WireValue::Structs(history) = value else {
        return Err("history type".into());
    };
    let mut next = history.clone();
    next.push(history.first().ok_or("history entry")?.clone());
    world.edit_batch(vec![WorldEdit::StructList {
        chunk: *b"PLYR",
        record: 0,
        path: vec![PathElement::Field("old_economy".into())],
        rows: next,
    }])?;
    let saved = world.saved_json()?;
    let reloaded = World::decode(&world.to_savegame()?)?;
    assert_eq!(saved, reloaded.saved_json()?);
    Ok(())
}

#[test]
fn oversized_history_rolls_back_earlier_coupled_edit() -> Result {
    let mut world = world()?;
    let before = world.saved_json()?;
    let table = world.tables().get(b"PLYR").ok_or("company table")?;
    let row = table.records().get(&0).ok_or("company")?;
    let value = table
        .schema()
        .fields()
        .iter()
        .zip(row.values())
        .find(|(f, _)| f.name() == "old_economy")
        .ok_or("history")?
        .1;
    let WireValue::Structs(history) = value else {
        return Err("history type".into());
    };
    let rows = vec![history.first().ok_or("history entry")?.clone(); 25];
    let result = world.edit_batch(vec![
        WorldEdit::Field {
            chunk: *b"PLYR",
            record: 0,
            path: vec![PathElement::Field("money".into())],
            value: WireValue::Signed(999_999),
        },
        WorldEdit::StructList {
            chunk: *b"PLYR",
            record: 0,
            path: vec![PathElement::Field("old_economy".into())],
            rows,
        },
    ]);
    assert!(result.is_err());
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
