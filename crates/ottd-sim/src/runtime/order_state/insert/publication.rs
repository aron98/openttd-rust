use super::vehicles::common;
use super::{RuntimeError, validation::Insertion};
use crate::runtime::order_state::{OrderState, view};
use ottd_save::{
    TableRecord, WireValue,
    world::{PathElement, World, WorldEdit},
};

pub(super) fn existing<'a>(
    world: &'a World,
    vehicle: view::Row<'_>,
) -> Result<Vec<view::Row<'a>>, RuntimeError> {
    let Some(list) = vehicle.number("orders")?.checked_sub(1) else {
        return Ok(Vec::new());
    };
    let (schema, rows) = view::row(
        world,
        *b"ORDL",
        u32::try_from(list).map_err(|_| RuntimeError::Invalid("order list"))?,
    )?
    .children("orders")?;
    Ok(rows
        .iter()
        .map(|record| view::Row { schema, record })
        .collect())
}
pub(super) fn publish(
    world: &mut World,
    state: &mut OrderState,
    args: &Insertion,
) -> Result<(), RuntimeError> {
    let vehicle = view::consist(world, args.vehicle.raw())?;
    let mut pool = state.lists.clone();
    let list = vehicle.number("orders")?.checked_sub(1);
    let list_id = match list {
        Some(id) => u32::try_from(id).map_err(|_| RuntimeError::Invalid("order list ID"))?,
        None => pool.allocate()?,
    };
    let rows = super::list::inserted(world, args)?;
    let count = u64::try_from(rows.len()).map_err(|_| RuntimeError::Invalid("order count"))?;
    let mut edits = Vec::new();
    if list.is_none() {
        edits.push(WorldEdit::InsertRecord {
            chunk: *b"ORDL",
            record: list_id,
            value: TableRecord::new(vec![WireValue::Structs(rows)]),
        });
        edits.push(common(
            args.vehicle.raw(),
            "orders",
            WireValue::Unsigned(u64::from(list_id) + 1),
        ));
    } else {
        edits.push(WorldEdit::StructList {
            chunk: *b"ORDL",
            record: list_id,
            path: vec![PathElement::Field("orders".into())],
            rows,
        });
    }
    let members = match list {
        None => vec![args.vehicle.raw()],
        Some(_) => world
            .derived()
            .order_lists
            .iter()
            .find(|l| l.id == list_id)
            .ok_or(RuntimeError::Invalid("shared order list"))?
            .vehicles
            .clone(),
    };
    edits.extend(super::vehicles::changes(
        world,
        members,
        (args.index, count),
    )?);
    if args.location != 0 {
        edits.push(crate::world_access::field_edit(
            *b"PLYR",
            u32::from(args.company),
            "last_build_coordinate",
            WireValue::Unsigned(u64::from(args.location)),
        ));
    }
    let pause = crate::world_access::unsigned(world, b"DATE", 0, "pause_mode")
        .map_err(|_| RuntimeError::Invalid("order pause mode"))?;
    if pause != 0 {
        edits.push(crate::world_access::field_edit(
            *b"DATE",
            0,
            "pause_mode",
            WireValue::Unsigned(pause | 128),
        ));
    }
    let mut tx = world.transaction();
    for edit in edits {
        tx.apply(edit)?;
    }
    let prepared = tx.prepare()?;
    let occupied = prepared
        .view()
        .table(*b"ORDL")
        .ok_or(RuntimeError::Invalid("order list candidate"))?
        .records()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    if occupied != pool.snapshot().occupied {
        return Err(RuntimeError::Invalid("order list allocation membership"));
    }
    prepared.commit();
    state.lists = pool;
    Ok(())
}
