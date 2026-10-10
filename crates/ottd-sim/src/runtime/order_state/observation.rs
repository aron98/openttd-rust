use super::{
    OrderState, RuntimeError,
    view::{self, OrderReader, Row},
};
use ottd_save::{WireValue, world::World};
use serde_json::{Value, json};
fn number(row: Row<'_>, name: &str) -> Result<Value, RuntimeError> {
    match row.field(name)? {
        WireValue::Signed(v) => Ok(json!(v)),
        WireValue::Unsigned(v) => Ok(json!(v)),
        _ => Err(RuntimeError::Invalid("order observation number")),
    }
}
fn reference(row: Row<'_>, name: &str) -> Result<Value, RuntimeError> {
    let id = row.number(name)?;
    Ok(if id == 0 {
        Value::Null
    } else {
        json!(id.saturating_sub(1))
    })
}
fn order(row: Row<'_>, prefix: &str) -> Result<Value, RuntimeError> {
    let mut value = serde_json::Map::new();
    for field in [
        "type",
        "flags",
        "dest",
        "refit_cargo",
        "wait_time",
        "travel_time",
        "max_speed",
    ] {
        value.insert(field.into(), number(row, &format!("{prefix}{field}"))?);
    }
    Ok(Value::Object(value))
}
fn orders(row: Row<'_>) -> Result<Vec<Value>, RuntimeError> {
    let (schema, rows) = row.children("orders")?;
    rows.iter()
        .map(|record| order(Row { schema, record }, ""))
        .collect()
}
fn consist(row: Row<'_>, backup: bool) -> Result<Value, RuntimeError> {
    let time = if backup {
        i32::from_le_bytes(
            u32::try_from(row.number("current_order_time")?)
                .map_err(|_| RuntimeError::Invalid("backup time bits"))?
                .to_le_bytes(),
        )
    } else {
        let WireValue::Signed(value) = row.field("current_order_time")? else {
            return Err(RuntimeError::Invalid("vehicle time"));
        };
        i32::try_from(*value).map_err(|_| RuntimeError::Invalid("vehicle time"))?
    };
    let WireValue::Bytes(name) = row.field("name")? else {
        return Err(RuntimeError::Invalid("consist name"));
    };
    let name = std::str::from_utf8(name).map_err(|_| RuntimeError::Invalid("consist name UTF8"))?;
    let mut value = serde_json::Map::new();
    value.insert("name".into(), json!(name));
    value.insert("current_order_time".into(), json!(time));
    value.insert(
        "current_order_time_bits".into(),
        json!(u32::from_le_bytes(time.to_le_bytes())),
    );
    for field in [
        "lateness_counter",
        "timetable_start",
        "service_interval",
        "cur_real_order_index",
        "cur_implicit_order_index",
        "vehicle_flags",
    ] {
        value.insert(field.into(), number(row, field)?);
    }
    Ok(Value::Object(value))
}
impl OrderState {
    pub(super) fn observe(&self, world: &World) -> Result<Value, RuntimeError> {
        let reader = OrderReader::Committed(world);
        let mut lists = Vec::new();
        let mut vehicles = Vec::new();
        let mut backups = Vec::new();
        for list in &world.derived().order_lists {
            let row = view::row(world, *b"ORDL", list.id)?;
            lists.push(json!({"id":list.id,"first_shared":list.first_shared,"members":list.vehicles,"num_vehicles":list.vehicles.len(),"num_manual_orders":list.num_manual_orders,"total_duration":list.total_duration,"timetable_duration":list.timetable_duration,"orders":orders(row)?}));
        }
        for (id, row) in reader.rows(*b"VEHS")? {
            let Some((_, common)) = view::vehicle(row)? else {
                return Err(RuntimeError::Unsupported(
                    "effect/disaster order observation",
                ));
            };
            vehicles.push(json!({"id":id,"type":row.number("type")?,"orders":reference(common,"orders")?,"current_order":order(common,"current_order.")?,"consist":consist(common,false)?}));
        }
        for (id, row) in reader.rows(*b"BKOR")? {
            backups.push(json!({"pool_slot":id,"id":self.backups.object_index(id)?,"user":row.number("user")?,"tile":row.number("tile")?,"group":row.number("group")?,"clone":reference(row,"clone")?,"orders":orders(row)?,"consist":consist(row,true)?}));
        }
        Ok(
            json!({"lists":lists,"vehicles":vehicles,"backups":backups,"list_pool":self.lists.snapshot(),"backup_pool":self.backups.snapshot()}),
        )
    }
}
