use super::*;
use crate::world::WorldEdit;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn fixture() -> Result<World> {
    let mut world = World::decode(&Savegame::decode(
        include_bytes!("../../../../../fixtures/world/populated-v362.sav"),
        crate::DEFAULT_MAX_BYTES,
    )?)?;
    let schema = world.tables().get(b"BKOR").ok_or("BKOR")?.schema();
    let row = TableRecord::new(
        schema
            .fields()
            .iter()
            .map(|f| match f.name() {
                "name" => WireValue::Bytes(b"live backup".to_vec()),
                "orders" => WireValue::Structs(Vec::new()),
                "lateness_counter" => WireValue::Signed(-7),
                "group" => WireValue::Unsigned(65534),
                "user" => WireValue::Unsigned(42),
                _ => WireValue::Unsigned(0),
            })
            .collect(),
    );
    world.edit_batch(vec![WorldEdit::InsertRecord {
        chunk: *b"BKOR",
        record: 7,
        value: row,
    }])?;
    Ok(world)
}
#[test]
fn backup_projection_omits_rows_without_mutating_live_or_offline_state() -> Result {
    // Given a nonempty live backup table.
    let world = fixture()?;
    let before = world.saved_json()?;
    let original = world.to_savegame()?;
    // When exporting the native single-player projection.
    let projected = world.without_order_backups().to_savegame()?;
    let decoded = World::decode(&projected)?;
    // Then only BKOR rows disappear; all other encoded chunks and live state remain.
    assert!(
        decoded
            .tables()
            .get(b"BKOR")
            .ok_or("BKOR")?
            .records()
            .is_empty()
    );
    for chunk in original.chunks().iter().filter(|c| c.id() != *b"BKOR") {
        let actual = projected
            .chunks()
            .iter()
            .find(|c| c.id() == chunk.id())
            .ok_or("chunk")?;
        assert_eq!(actual.body(), chunk.body());
        assert_eq!(actual.kind(), chunk.kind());
    }
    assert_eq!(
        world.without_order_backups().saved_json()?,
        decoded.saved_json()?
    );
    assert_eq!(world.saved_json()?, before);
    assert_eq!(
        world.to_savegame()?.encode(Compression::None)?,
        original.encode(Compression::None)?
    );
    assert_eq!(World::decode(&original)?.saved_json()?, before);
    Ok(())
}
