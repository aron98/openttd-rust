use crate::{
    commands::CommandError,
    world_access::{row_field, unsigned},
};
use ottd_save::{TableRecord, WireValue, world::World};
use std::collections::BTreeSet;

pub(super) fn nearest_town(world: &World, tile: u32) -> Result<Option<u32>, CommandError> {
    let towns = world
        .tables()
        .get(b"CITY")
        .ok_or(CommandError::Unsupported("town table"))?;
    let width = std::num::NonZeroU32::new(world.map().width())
        .ok_or(CommandError::Unsupported("map width"))?;
    let mut nearest = None;
    for id in towns.records().keys() {
        let xy = u32::try_from(unsigned(world, b"CITY", *id, "xy")?)
            .map_err(|_| CommandError::Overflow("town tile"))?;
        let distance = (tile % width)
            .abs_diff(xy % width)
            .checked_add((tile / width).abs_diff(xy / width))
            .ok_or(CommandError::Overflow("town distance"))?;
        let key = (distance, *id);
        if nearest.is_none_or(|old| key < old) {
            nearest = Some(key);
        }
    }
    Ok(nearest.map(|(_, id)| id))
}

pub(super) fn build(world: &World, tile: u32) -> Result<TableRecord, CommandError> {
    let town = nearest_town(world, tile)?.map_or(0, |id| u64::from(id) + 1);
    let table = world
        .tables()
        .get(b"DEPT")
        .ok_or(CommandError::Unsupported("depot table"))?;
    let mut used = BTreeSet::new();
    for row in table.records().values() {
        if row_field(table.schema(), row, "town")? != &WireValue::Unsigned(town) {
            continue;
        }
        let WireValue::Unsigned(xy) = row_field(table.schema(), row, "xy")? else {
            return Err(CommandError::Unsupported("depot tile field"));
        };
        let source = usize::try_from(*xy)
            .ok()
            .and_then(|index| world.map().tiles().get(index))
            .ok_or(CommandError::Unsupported("depot tile"))?;
        if source.tile_type() >> 4 != 2 {
            continue;
        }
        let WireValue::Unsigned(number) = row_field(table.schema(), row, "town_cn")? else {
            return Err(CommandError::Unsupported("depot number field"));
        };
        used.insert(u16::try_from(*number).map_err(|_| CommandError::Overflow("depot number"))?);
    }
    let number = (0..u16::MAX)
        .find(|n| !used.contains(n))
        .ok_or(CommandError::Overflow("depot naming space"))?;
    let date = crate::world_access::signed(world, b"DATE", 0, "date")?;
    let mut values = Vec::with_capacity(5);
    for field in table.schema().fields() {
        values.push(match field.name() {
            "xy" => WireValue::Unsigned(u64::from(tile)),
            "town" => WireValue::Unsigned(town),
            "town_cn" => WireValue::Unsigned(u64::from(number)),
            "name" => WireValue::Bytes(Vec::new()),
            "build_date" => WireValue::Signed(date),
            _ => return Err(CommandError::Unsupported("depot constructor schema")),
        });
    }
    if values.len() != 5 {
        return Err(CommandError::Unsupported("depot constructor fields"));
    }
    Ok(TableRecord::new(values))
}
