use super::{CommandCost, CommandError, Plan};
use crate::world_access::{WorldAccessError, row_field, unsigned};
use ottd_save::{
    TableRecord, TableSchema, WireValue,
    world::{PathElement, World, WorldEdit},
};

pub(super) fn service_interval(
    world: &World,
    company: u8,
    vehicle: u32,
    interval: u16,
    custom: bool,
    percent: bool,
) -> Result<Plan, CommandError> {
    let table = world
        .tables()
        .get(b"VEHS")
        .ok_or_else(|| WorldAccessError("VEHS".into()))?;
    let Some(row) = table.records().get(&vehicle) else {
        return Ok(Plan::empty(CommandCost::failure("CMD_ERROR")));
    };
    if number(table.schema(), row, "type")? != 1 {
        return Err(CommandError::Unsupported(
            "service interval for non-road vehicle",
        ));
    }
    let (schema, row) = child(table.schema(), row, "roadveh")?;
    let (schema, row) = child(schema, row, "common")?;
    if number(schema, row, "subtype")? & 1 == 0 {
        return Ok(Plan::empty(CommandCost::failure("CMD_ERROR")));
    }
    let owner = number(schema, row, "owner")?;
    if owner != u64::from(company) {
        let mut cost = CommandCost::failure("STR_ERROR_OWNED_BY");
        cost.error_params = vec![
            0x881D,
            i64::try_from(owner).map_err(|_| CommandError::Overflow("vehicle owner"))?,
        ];
        return Ok(Plan::empty(cost));
    }
    let (interval, percent) = if custom {
        let (min, max) = if percent {
            (5, 90)
        } else if unsigned(world, b"PATS", 0, "economy.timekeeping_units")? == 1 {
            (1, 30)
        } else {
            (30, 800)
        };
        if !(min..=max).contains(&interval) {
            return Ok(Plan::empty(CommandCost::failure("CMD_ERROR")));
        }
        (u64::from(interval), percent)
    } else {
        let companies = world
            .tables()
            .get(b"PLYR")
            .ok_or_else(|| WorldAccessError("PLYR".into()))?;
        let company_row = companies
            .records()
            .get(&u32::from(company))
            .ok_or_else(|| WorldAccessError("company".into()))?;
        let (schema, row) = child(companies.schema(), company_row, "settings")?;
        (
            number(schema, row, "settings.vehicle.servint_roadveh")?,
            number(schema, row, "settings.vehicle.servint_ispercent")? != 0,
        )
    };
    let flags = (number(schema, row, "vehicle_flags")? & !0x300)
        | (u64::from(custom) << 8)
        | (u64::from(percent) << 9);
    let edit = |name: &str, value| WorldEdit::Field {
        chunk: *b"VEHS",
        record: vehicle,
        path: vec![
            PathElement::Field("roadveh".into()),
            PathElement::Index(0),
            PathElement::Field("common".into()),
            PathElement::Index(0),
            PathElement::Field(name.into()),
        ],
        value: WireValue::Unsigned(value),
    };
    Ok(Plan {
        cost: CommandCost::success(0, 255),
        edits: vec![
            edit("service_interval", interval),
            edit("vehicle_flags", flags),
        ],
        returns: None,
    })
}
fn number(schema: &TableSchema, row: &TableRecord, name: &str) -> Result<u64, CommandError> {
    match row_field(schema, row, name)? {
        WireValue::Unsigned(v) => Ok(*v),
        WireValue::Signed(v) => u64::try_from(*v).map_err(|_| WorldAccessError(name.into()).into()),
        WireValue::Bytes(_) | WireValue::Array(_) | WireValue::Structs(_) => {
            Err(WorldAccessError(name.into()).into())
        }
    }
}
fn child<'a>(
    schema: &'a TableSchema,
    row: &'a TableRecord,
    name: &str,
) -> Result<(&'a TableSchema, &'a TableRecord), CommandError> {
    let schema_child = schema
        .fields()
        .iter()
        .find(|f| f.name() == name)
        .and_then(ottd_save::FieldSchema::child)
        .ok_or_else(|| WorldAccessError(name.into()))?;
    let WireValue::Structs(rows) = row_field(schema, row, name)? else {
        return Err(WorldAccessError(name.into()).into());
    };
    let [row] = rows.as_slice() else {
        return Err(WorldAccessError(name.into()).into());
    };
    Ok((schema_child, row))
}
