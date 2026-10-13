//! Native temporary-engine loading requires occupied IDs to form a zero-based prefix.
use ottd_save::{
    Savegame,
    world::{PathElement, World, WorldEdit},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn world() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/world/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}

fn rollback(mut world: World, edit: WorldEdit) -> Result {
    let before = world.saved_json()?;
    let derived = serde_json::to_value(world.derived())?;
    let result = world.edit_batch(vec![
        WorldEdit::Field {
            chunk: *b"PLYR",
            record: 0,
            path: vec![PathElement::Field("money".into())],
            value: ottd_save::WireValue::Signed(999_999),
        },
        edit,
    ]);
    assert!(
        result.is_err(),
        "accepted engine gap and published the earlier money edit"
    );
    assert_eq!(world.saved_json()?, before);
    assert_eq!(serde_json::to_value(world.derived())?, derived);
    Ok(())
}

#[test]
fn deleting_interior_engine_rolls_back_entire_transaction() -> Result {
    rollback(
        world()?,
        WorldEdit::RemoveRecord {
            chunk: *b"ENGN",
            record: 1,
        },
    )
}

#[test]
fn inserting_gapped_engine_rolls_back_entire_transaction() -> Result {
    let world = world()?;
    let value = world
        .tables()
        .get(b"ENGN")
        .and_then(|table| table.records().get(&0))
        .ok_or("engine")?
        .clone();
    rollback(
        world,
        WorldEdit::InsertRecord {
            chunk: *b"ENGN",
            record: 1000,
            value,
        },
    )
}

#[test]
fn direct_world_decode_rejects_engine_gaps() -> Result {
    let world = world()?;
    let base = world.tables().get(b"ENGN").ok_or("engines")?;
    let mut accepted = Vec::new();
    for hole in [0, 1, 1000] {
        let mut table = base.clone();
        if hole == 1000 {
            let value = table.records().get(&0).ok_or("engine")?.clone();
            table.records_mut().insert(hole, value);
        } else {
            table
                .records_mut()
                .remove(&hole)
                .ok_or("engine to remove")?;
        }
        let mut save = world.to_savegame()?;
        save.replace_chunk(table.encode()?)?;
        if World::decode(&save).is_ok() {
            accepted.push(hole);
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted malformed ENGN indices: {accepted:?}"
    );
    Ok(())
}

#[test]
fn final_contiguous_engine_mutations_remain_supported() -> Result {
    let mut world = world()?;
    let before = world.saved_json()?;
    let table = world.tables().get(b"ENGN").ok_or("engines")?;
    let engine = table.records().get(&1).ok_or("engine")?.clone();
    let length = u32::try_from(table.records().len())?;
    world.edit_batch(vec![
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
            value: engine.clone(),
        },
    ])?;
    assert_eq!(world.saved_json()?, before);
    world.edit_batch(vec![WorldEdit::InsertRecord {
        chunk: *b"ENGN",
        record: length,
        value: engine,
    }])?;
    assert_eq!(
        world
            .tables()
            .get(b"ENGN")
            .ok_or("engines")?
            .records()
            .len(),
        usize::try_from(length.checked_add(1).ok_or("length")?)?
    );
    world.edit_batch(vec![WorldEdit::RemoveRecord {
        chunk: *b"ENGN",
        record: length,
    }])?;
    assert_eq!(world.saved_json()?, before);
    let removal: Vec<_> = (1..length)
        .map(|record| WorldEdit::RemoveRecord {
            chunk: *b"ENGN",
            record,
        })
        .collect();
    world.edit_batch(removal)?;
    assert_eq!(
        world
            .tables()
            .get(b"ENGN")
            .ok_or("engines")?
            .records()
            .len(),
        1
    );
    world.edit_batch(vec![WorldEdit::RemoveRecord {
        chunk: *b"ENGN",
        record: 0,
    }])?;
    assert!(
        world
            .tables()
            .get(b"ENGN")
            .ok_or("engines")?
            .records()
            .is_empty()
    );
    assert_eq!(
        World::decode(&world.to_savegame()?)?.saved_json()?,
        world.saved_json()?
    );
    Ok(())
}
