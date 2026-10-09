//! Candidate reads, poisoned transactions, prepared rollback and publication.
use ottd_save::{
    Savegame, TileRawParts, WireValue,
    world::{PathElement, World, WorldEdit},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn world() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/world/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
fn money(value: i64) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"PLYR",
        record: 0,
        path: vec![PathElement::Field("money".into())],
        value: WireValue::Signed(value),
    }
}
#[test]
fn candidate_reads_see_prior_writes_and_drop_rolls_back() -> Result {
    let mut world = world()?;
    let before = world.saved_json()?;
    let derived = serde_json::to_value(world.derived())?;
    let old_tile = world.map().tiles().first().ok_or("tile")?.clone();
    {
        let mut transaction = world.transaction();
        transaction.apply(money(123))?;
        transaction.apply(money(456))?;
        let table = transaction.view().table(*b"PLYR").ok_or("table")?;
        let row = table.record(0).ok_or("company")?;
        let value = table
            .schema()
            .fields()
            .iter()
            .zip(row.values())
            .find(|(field, _)| field.name() == "money")
            .ok_or("money")?
            .1;
        assert_eq!(value, &WireValue::Signed(456));
        let mut tile = TileRawParts::from(&old_tile);
        tile.height = tile.height.wrapping_add(1);
        transaction.apply(WorldEdit::Tile {
            index: 0,
            value: tile.into(),
        })?;
        assert_eq!(transaction.view().tile(0)?.height(), tile.height);
        transaction.apply(WorldEdit::Tile {
            index: 0,
            value: old_tile,
        })?;
        assert_eq!(transaction.touched(), (1, 1));
    }
    assert_eq!(world.saved_json()?, before);
    assert_eq!(serde_json::to_value(world.derived())?, derived);
    Ok(())
}
#[test]
fn failed_apply_poison_and_failed_prepare_restore_everything() -> Result {
    let mut world = world()?;
    let before = world.saved_json()?;
    let derived = serde_json::to_value(world.derived())?;
    {
        let mut transaction = world.transaction();
        transaction.apply(money(123))?;
        assert!(
            transaction
                .apply(WorldEdit::RemoveRecord {
                    chunk: *b"CAPA",
                    record: 999
                })
                .is_err()
        );
        assert!(transaction.apply(money(456)).is_err());
        assert!(transaction.prepare().is_err());
    }
    assert_eq!(world.saved_json()?, before);
    {
        let mut transaction = world.transaction();
        transaction.apply(money(789))?;
        transaction.apply(WorldEdit::RemoveRecord {
            chunk: *b"ENGN",
            record: 1,
        })?;
        assert!(transaction.prepare().is_err());
    }
    assert_eq!(world.saved_json()?, before);
    assert_eq!(serde_json::to_value(world.derived())?, derived);
    Ok(())
}
#[test]
fn discarding_prepared_candidate_preserves_world_and_map_storage() -> Result {
    let mut world = world()?;
    let before = world.saved_json()?;
    let derived = serde_json::to_value(world.derived())?;
    let map_storage = world.map().tiles().as_ptr();
    {
        let mut transaction = world.transaction();
        transaction.apply(money(555))?;
        let prepared = transaction.prepare()?;
        assert_eq!(serde_json::to_value(prepared.derived())?, derived);
        assert_eq!(prepared.map().width(), 256);
        assert!(std::ptr::eq(
            prepared.map().tile(0).ok_or("tile")?,
            map_storage
        ));
        drop(prepared);
    }
    assert_eq!(world.saved_json()?, before);
    assert_eq!(serde_json::to_value(world.derived())?, derived);
    assert_eq!(world.map().tiles().as_ptr(), map_storage);
    Ok(())
}
#[test]
fn lifecycle_changes_validate_only_the_final_candidate() -> Result {
    let mut world = world()?;
    let original = world
        .tables()
        .get(b"ENGN")
        .and_then(|table| table.records().get(&1))
        .ok_or("engine")?
        .clone();
    let expected = {
        let mut expected = world.clone();
        expected.edit_batch(vec![money(222)])?;
        expected.saved_json()?
    };
    let mut transaction = world.transaction();
    transaction.apply(WorldEdit::RemoveRecord {
        chunk: *b"ENGN",
        record: 1,
    })?;
    assert!(
        transaction
            .view()
            .table(*b"ENGN")
            .ok_or("engines")?
            .record(1)
            .is_none()
    );
    transaction.apply(WorldEdit::InsertRecord {
        chunk: *b"ENGN",
        record: 1,
        value: original.clone(),
    })?;
    transaction.apply(WorldEdit::ReplaceRecord {
        chunk: *b"ENGN",
        record: 1,
        value: original,
    })?;
    transaction.apply(money(222))?;
    assert_eq!(transaction.touched(), (2, 0));
    transaction.prepare()?.commit();
    assert_eq!(world.saved_json()?, expected);
    assert_eq!(
        World::decode(&world.to_savegame()?)?.saved_json()?,
        expected
    );
    Ok(())
}

#[test]
fn forgetting_public_candidate_or_prepared_state_never_changes_world() -> Result {
    let mut world = world()?;
    let before = world.saved_json()?;
    let derived = serde_json::to_value(world.derived())?;
    let mut tile = TileRawParts::from(world.map().tiles().first().ok_or("tile")?);
    tile.height = tile.height.wrapping_add(1);
    let mut transaction = world.transaction();
    transaction.apply(money(123))?;
    transaction.apply(WorldEdit::Tile {
        index: 0,
        value: tile.into(),
    })?;
    transaction.apply(WorldEdit::RemoveRecord {
        chunk: *b"ENGN",
        record: 1,
    })?;
    std::mem::forget(transaction);
    assert_eq!(world.saved_json()?, before);
    let mut transaction = world.transaction();
    transaction.apply(money(456))?;
    transaction.apply(WorldEdit::Tile {
        index: 0,
        value: tile.into(),
    })?;
    std::mem::forget(transaction.prepare()?);
    assert_eq!(world.saved_json()?, before);
    assert_eq!(serde_json::to_value(world.derived())?, derived);
    assert_eq!(World::decode(&world.to_savegame()?)?.saved_json()?, before);
    Ok(())
}

#[test]
fn candidate_records_merge_insertions_and_deletions_in_native_order() -> Result {
    let mut world = world()?;
    let row = world
        .tables()
        .get(b"CAPA")
        .and_then(|table| table.records().get(&2))
        .ok_or("packet")?
        .clone();
    let mut tx = world.transaction();
    tx.apply(WorldEdit::RemoveRecord {
        chunk: *b"CAPA",
        record: 2,
    })?;
    tx.apply(WorldEdit::InsertRecord {
        chunk: *b"CAPA",
        record: 5,
        value: row.clone(),
    })?;
    tx.apply(WorldEdit::InsertRecord {
        chunk: *b"CAPA",
        record: 1000,
        value: row,
    })?;
    tx.apply(WorldEdit::RemoveRecord {
        chunk: *b"CAPA",
        record: 1000,
    })?;
    let ids: Vec<_> = tx
        .view()
        .table(*b"CAPA")
        .ok_or("cargo table")?
        .records()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids, vec![0, 1, 3, 4, 5, 6]);
    Ok(())
}
