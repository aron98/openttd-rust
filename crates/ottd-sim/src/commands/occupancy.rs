use super::{CommandCost, CommandError, landscape};
use crate::world_access::{WorldAccessError, row_field};
use ottd_save::{TableRecord, TableSchema, WireValue, world::World};

pub(super) fn occupied(world: &World, tile: u32) -> Result<Option<CommandCost>, CommandError> {
    let Some(table) = world.tables().get(b"VEHS") else {
        return Ok(None);
    };
    let ground_z = u64::from(landscape::tile_at(world, tile)?.height()) * 8;
    let mut failure = None;
    for row in table.records().values() {
        let kind = number(row_field(table.schema(), row, "type")?)?;
        let key = match kind {
            0 => "train",
            1 => "roadveh",
            2 => "ship",
            3 => "aircraft",
            4 => "effect",
            5 => continue,
            _ => return Err(CommandError::Unsupported("vehicle type")),
        };
        let (schema, row) = child(table.schema(), row, key)?;
        let (schema, row) = if kind < 4 {
            child(schema, row, "common")?
        } else {
            (schema, row)
        };
        let position = number(row_field(schema, row, "tile")?)?;
        let height = number(row_field(schema, row, "z_pos")?)?;
        let subtype = number(row_field(schema, row, "subtype")?)?;
        if position != u64::from(tile) || height > ground_z || (kind == 3 && subtype == 4) {
            continue;
        }
        let symbol = match kind {
            0 => "STR_ERROR_TRAIN_IN_THE_WAY",
            1 => "STR_ERROR_ROAD_VEHICLE_IN_THE_WAY",
            2 => "STR_ERROR_SHIP_IN_THE_WAY",
            3 => "STR_ERROR_AIRCRAFT_IN_THE_WAY",
            _ => return Err(CommandError::Unsupported("effect vehicle occupancy")),
        };
        if failure.is_some_and(|old| old != symbol) {
            return Err(CommandError::Unsupported(
                "ambiguous native vehicle hash error order",
            ));
        }
        failure = Some(symbol);
    }
    Ok(failure.map(CommandCost::failure))
}
fn child<'a>(
    schema: &'a TableSchema,
    row: &'a TableRecord,
    name: &str,
) -> Result<(&'a TableSchema, &'a TableRecord), CommandError> {
    let field = schema
        .fields()
        .iter()
        .find(|f| f.name() == name)
        .ok_or_else(|| WorldAccessError(name.into()))?;
    let WireValue::Structs(rows) = row_field(schema, row, name)? else {
        return Err(WorldAccessError(name.into()).into());
    };
    let schema = field.child().ok_or_else(|| WorldAccessError(name.into()))?;
    let row = rows.first().ok_or_else(|| WorldAccessError(name.into()))?;
    Ok((schema, row))
}
fn number(value: &WireValue) -> Result<u64, CommandError> {
    match value {
        WireValue::Unsigned(v) => Ok(*v),
        WireValue::Signed(v) => {
            u64::try_from(*v).map_err(|_| CommandError::Unsupported("negative vehicle coordinate"))
        }
        WireValue::Bytes(_) | WireValue::Array(_) | WireValue::Structs(_) => {
            Err(CommandError::Unsupported("vehicle number"))
        }
    }
}
