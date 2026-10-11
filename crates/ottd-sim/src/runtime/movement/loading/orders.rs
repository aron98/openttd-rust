//! Ordinary-order timetable measurement and index advancement, in native phase order.
use super::{State, VehicleId, WorldTickError, number, records, set, unsupported};
use crate::runtime::stations::saved::StationRow;
use ottd_save::{
    WireValue,
    world::{PathElement, WorldEdit},
};
pub(super) fn cursor(
    state: &State<'_>,
    id: VehicleId,
) -> Result<(u32, usize, usize), WorldTickError> {
    let list = u32::try_from(
        number(state, id, false, "orders")?
            .checked_sub(1)
            .ok_or_else(|| unsupported("road_loading", "order list"))?,
    )
    .map_err(|_| unsupported("road_loading", "order list ID"))?;
    let (schema, rows) = records::row(state, *b"ORDL", list)?.rows("orders")?;
    for record in rows {
        let row = StationRow { schema, record };
        if row.number("type")? != 33
            || row.number("flags")? != 0
            || row.number("refit_cargo")? != 254
        {
            return Err(unsupported("road_loading", "ordinary manual order list"));
        }
    }
    let index = usize::try_from(number(state, id, false, "cur_real_order_index")?)
        .map_err(|_| unsupported("road_loading", "order index"))?;
    if index >= rows.len() {
        return Err(unsupported("road_loading", "order index outside list"));
    }
    Ok((list, index, rows.len()))
}
pub(super) fn measure(
    state: &mut State<'_>,
    id: VehicleId,
    travelling: bool,
) -> Result<(), WorldTickError> {
    let (list, index, _) = cursor(state, id)?;
    let time = number(state, id, false, "current_order_time")?;
    set(state, id, false, "current_order_time", 0)?;
    let flags = number(state, id, false, "vehicle_flags")?;
    if number(state, id, false, "timetable_start")? != 0
        || number(state, id, false, "lateness_counter")? != 0
        || flags & !0x0109 != 0
    {
        return Err(unsupported(
            "road_loading",
            "scheduled or autofill timetable",
        ));
    }
    let just_started = travelling && index == 0 && flags & 8 == 0;
    let started = flags & 8 != 0 || travelling && index == 0;
    if travelling && index == 0 {
        set(state, id, false, "vehicle_flags", flags | 8)?;
    }
    if !started {
        return Ok(());
    }
    if travelling {
        set(state, id, false, "current_order.wait_time", 0)?;
    }
    if just_started {
        return Ok(());
    }
    let field = if travelling {
        "travel_time"
    } else {
        "wait_time"
    };
    let value = time
        .max(1)
        .wrapping_add(36)
        .wrapping_div(37)
        .wrapping_mul(37)
        & 65535;
    state.apply(WorldEdit::Field {
        chunk: *b"ORDL",
        record: list,
        path: vec![
            PathElement::Field("orders".into()),
            PathElement::Index(index),
            PathElement::Field(field.into()),
        ],
        value: WireValue::Unsigned(
            u64::try_from(value).map_err(|_| unsupported("road_loading", "measured duration"))?,
        ),
    })
}
pub(super) fn advance(state: &mut State<'_>, id: VehicleId) -> Result<(), WorldTickError> {
    let (_, real, len) = cursor(state, id)?;
    let implicit = usize::try_from(number(state, id, false, "cur_implicit_order_index")?)
        .map_err(|_| unsupported("road_loading", "implicit index"))?;
    if implicit >= len {
        return Err(unsupported("road_loading", "implicit index outside list"));
    }
    let next = if implicit == real {
        real.wrapping_add(1)
            .checked_rem(len)
            .ok_or_else(|| unsupported("road_loading", "empty orders"))?
    } else {
        real
    };
    let next = i64::try_from(next).map_err(|_| unsupported("road_loading", "order index"))?;
    set(state, id, false, "cur_real_order_index", next)?;
    set(state, id, false, "cur_implicit_order_index", next)
}
pub(super) fn destinations(state: &State<'_>, id: VehicleId) -> Result<Vec<u32>, WorldTickError> {
    let (list, _, _) = cursor(state, id)?;
    let (schema, rows) = records::row(state, *b"ORDL", list)?.rows("orders")?;
    rows.iter()
        .map(|record| {
            u32::try_from((StationRow { schema, record }).number("dest")?)
                .map_err(|_| unsupported("road_loading", "station destination"))
        })
        .collect()
}
