use super::{RuntimeError, publication::existing, validation::Insertion};
use crate::runtime::order_state::view;
use ottd_save::{TableRecord, WireValue, world::World};
pub(super) fn inserted(world: &World, args: &Insertion) -> Result<Vec<TableRecord>, RuntimeError> {
    let vehicle = view::consist(world, args.vehicle.raw())?;
    let table = view::table(world, *b"ORDL")?;
    let schema = table
        .schema()
        .fields()
        .iter()
        .find(|f| f.name() == "orders")
        .and_then(ottd_save::FieldSchema::child)
        .ok_or(RuntimeError::Invalid("order schema"))?;
    let mut rows = existing(world, vehicle)?
        .iter()
        .map(|r| r.record.clone())
        .collect::<Vec<_>>();
    let order = args.order;
    let values = schema
        .fields()
        .iter()
        .map(|field| {
            Ok(WireValue::Unsigned(match field.name() {
                "type" => u64::from(order.kind),
                "flags" => u64::from(order.flags),
                "dest" => u64::from(order.dest),
                "refit_cargo" => u64::from(order.refit_cargo),
                "wait_time" => u64::from(order.wait_time),
                "travel_time" => u64::from(order.travel_time),
                "max_speed" => u64::from(order.max_speed),
                _ => return Err(RuntimeError::Invalid("order constructor field")),
            }))
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    rows.insert(usize::from(args.index), TableRecord::new(values));
    let count = std::num::NonZeroU64::new(
        u64::try_from(rows.len()).map_err(|_| RuntimeError::Invalid("order count"))?,
    )
    .ok_or(RuntimeError::Invalid("empty inserted orders"))?;
    for (position, record) in rows.iter_mut().enumerate() {
        let row = view::Row { schema, record };
        if row.number("type")? & 15 != 7 {
            continue;
        }
        let target = row.number("flags")?;
        let next = if usize::try_from(target) == Ok(position) {
            target.saturating_add(1) % count
        } else if target >= u64::from(args.index) {
            target.wrapping_add(1) & 255
        } else {
            target
        };
        let values = schema
            .fields()
            .iter()
            .zip(record.values())
            .map(|(field, value)| {
                if field.name() == "flags" {
                    WireValue::Unsigned(next)
                } else {
                    value.clone()
                }
            })
            .collect();
        *record = TableRecord::new(values);
    }
    Ok(rows)
}
