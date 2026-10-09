//! Native schema, pool, map and script corruption rejection.
#![cfg(test)]
use ottd_save::{
    Compression, Savegame, WireValue,
    world::{World, WorldError},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn world() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/world/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}

#[test]
fn rejects_renamed_reference_descriptor_in_real_save() -> Result {
    let mut bytes = world()?.encode(Compression::None)?;
    let needle = b"cargo.packets";
    let offset = bytes
        .windows(needle.len())
        .position(|w| w == needle)
        .ok_or("reference descriptor")?;
    *bytes.get_mut(offset).ok_or("descriptor byte")? = b'C';
    let save = Savegame::decode(&bytes, ottd_save::DEFAULT_MAX_BYTES)?;
    assert!(matches!(
        World::decode(&save),
        Err(WorldError::Invalid {
            reason: "schema differs from pinned native v362 descriptors",
            ..
        })
    ));
    Ok(())
}

#[test]
fn rejects_unknown_chunk_in_real_save() -> Result {
    let mut bytes = world()?.encode(Compression::None)?;
    let offset = bytes
        .windows(5)
        .position(|w| w == b"MAPS\x03")
        .ok_or("map chunk")?;
    *bytes.get_mut(offset).ok_or("chunk byte")? = b'Z';
    let save = Savegame::decode(&bytes, ottd_save::DEFAULT_MAX_BYTES)?;
    assert!(matches!(
        World::decode(&save),
        Err(WorldError::Invalid {
            reason: "unknown or obsolete v362 chunk",
            ..
        })
    ));
    Ok(())
}

#[test]
fn rejects_native_pool_limit_even_when_wire_table_is_valid() -> Result {
    for (chunk, id) in [(*b"PLYR", 15), (*b"GRPS", 64000), (*b"ENGN", 64000)] {
        let world = world()?;
        let mut save = world.to_savegame()?;
        let mut table = world.tables().get(&chunk).ok_or("pool")?.clone();
        let record = table
            .records()
            .values()
            .next()
            .ok_or("populated pool")?
            .clone();
        table.records_mut().insert(id, record);
        save.replace_chunk(table.encode()?)?;
        assert!(matches!(
            World::decode(&save),
            Err(WorldError::Invalid {
                reason: "pool ID out of range",
                ..
            })
        ));
    }
    Ok(())
}

#[test]
fn rejects_native_fixed_array_length_even_when_wire_table_is_valid() -> Result {
    let world = world()?;
    let mut save = world.to_savegame()?;
    let mut table = world.tables().get(b"PLYR").ok_or("company pool")?.clone();
    let position = table
        .schema()
        .fields()
        .iter()
        .position(|f| f.name() == "yearly_expenses")
        .ok_or("expense descriptor")?;
    let row = table.records_mut().get_mut(&1).ok_or("company")?;
    *row.values_mut().get_mut(position).ok_or("expenses")? =
        WireValue::Array(vec![WireValue::Signed(0)]);
    save.replace_chunk(table.encode()?)?;
    assert!(matches!(
        World::decode(&save),
        Err(WorldError::Invalid {
            reason: "invalid native fixed array length",
            ..
        })
    ));
    Ok(())
}

#[test]
fn rejects_station_tile_with_missing_station_pool_id() -> Result {
    let mut world = world()?;
    let (index, tile) = world
        .map()
        .tiles()
        .iter()
        .enumerate()
        .find(|(_, t)| t.tile_type() >> 4 == 5)
        .ok_or("station tile")?;
    let mut json = serde_json::to_value(tile)?;
    json.as_object_mut()
        .ok_or("tile object")?
        .insert("m2".to_owned(), serde_json::json!(63999));
    let tile = serde_json::from_value(json)?;
    assert!(matches!(
        world.edit_tile(u32::try_from(index)?, &tile),
        Err(WorldError::Invalid {
            reason: "dangling object reference",
            ..
        })
    ));
    Ok(())
}

#[test]
fn roundtrips_extended_script_and_mod_state_without_execution() -> Result {
    for bytes in [
        include_bytes!("../../../fixtures/world/populated-extended-v362.sav").as_slice(),
        include_bytes!("../../../fixtures/world/modded-v362.sav").as_slice(),
    ] {
        let world = World::decode(&Savegame::decode(bytes, ottd_save::DEFAULT_MAX_BYTES)?)?;
        let encoded = world.encode(Compression::Zlib)?;
        let reloaded = World::decode(&Savegame::decode(&encoded, ottd_save::DEFAULT_MAX_BYTES)?)?;
        assert_eq!(world.saved_json()?, reloaded.saved_json()?);
        assert_eq!(world.derived(), reloaded.derived());
    }
    Ok(())
}
