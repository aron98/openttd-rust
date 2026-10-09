use crate::{
    commands::{CommandCost, CommandError},
    world_access::{WorldAccessError, row_field},
};
use ottd_core::terrain::Corner;
use ottd_save::{TableRecord, TableSchema, WireValue, world::World};

pub(super) fn occupied(world: &World, tile: u32) -> Result<Option<CommandCost>, CommandError> {
    let (slope, base) = crate::terrain::tile_slope_z(world, tile)
        .map_err(|_| CommandError::Unsupported("depot ground geometry"))?;
    let mut maximum = 0;
    for corner in [Corner::North, Corner::West, Corner::East, Corner::South] {
        maximum = maximum.max(
            slope
                .corner_z(corner)
                .map_err(|_| CommandError::Unsupported("depot corner"))?,
        );
    }
    let ground = u64::from(base)
        .saturating_add(u64::from(maximum))
        .saturating_mul(8);
    let table = world
        .tables()
        .get(b"VEHS")
        .ok_or(CommandError::Unsupported("vehicle table"))?;
    let mut failure = None;
    for row in table.records().values() {
        let kind = number(table.schema(), row, "type")?;
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
        if number(schema, row, "tile")? != u64::from(tile)
            || number(schema, row, "z_pos")? > ground
            || (kind == 3 && number(schema, row, "subtype")? == 4)
        {
            continue;
        }
        let symbol = match kind {
            0 => "STR_ERROR_TRAIN_IN_THE_WAY",
            1 => "STR_ERROR_ROAD_VEHICLE_IN_THE_WAY",
            2 => "STR_ERROR_SHIP_IN_THE_WAY",
            3 => "STR_ERROR_AIRCRAFT_IN_THE_WAY",
            _ => return Err(CommandError::Unsupported("effect vehicle ground occupancy")),
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
fn number(schema: &TableSchema, row: &TableRecord, name: &str) -> Result<u64, CommandError> {
    match row_field(schema, row, name)? {
        WireValue::Unsigned(v) => Ok(*v),
        WireValue::Signed(v) => {
            u64::try_from(*v).map_err(|_| CommandError::Unsupported("negative vehicle coordinate"))
        }
        _ => Err(CommandError::Unsupported("vehicle number")),
    }
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
    Ok((
        field.child().ok_or_else(|| WorldAccessError(name.into()))?,
        rows.first().ok_or_else(|| WorldAccessError(name.into()))?,
    ))
}
