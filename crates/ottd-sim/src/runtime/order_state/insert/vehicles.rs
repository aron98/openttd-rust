use super::RuntimeError;
use crate::runtime::order_state::view;
use ottd_save::{
    WireValue,
    world::{PathElement, World, WorldEdit},
};
pub(super) fn changes(
    world: &World,
    members: Vec<u32>,
    insertion: (u8, u64),
) -> Result<Vec<WorldEdit>, RuntimeError> {
    let (position, count) = insertion;
    let mut edits = Vec::new();
    for id in members {
        let source = view::row(world, *b"VEHS", id)?;
        let Some(("roadveh", current)) = view::vehicle(source)? else {
            return Err(RuntimeError::Unsupported("shared non-road insertion"));
        };
        let (schema, records) = source.children("roadveh")?;
        let [record] = records else {
            return Err(RuntimeError::Invalid("road vehicle variant"));
        };
        let road = view::Row { schema, record };
        if u64::from(position) == current.number("cur_implicit_order_index")? {
            edits.push(WorldEdit::Field {
                chunk: *b"VEHS",
                record: id,
                path: vec![
                    PathElement::Field("roadveh".into()),
                    PathElement::Index(0),
                    PathElement::Field("gv_flags".into()),
                ],
                value: WireValue::Unsigned(road.number("gv_flags")? | 4),
            });
        }
        for name in ["cur_real_order_index", "cur_implicit_order_index"] {
            let old = current.number(name)?;
            if u64::from(position) <= old && old.saturating_add(1) < count {
                edits.push(common(id, name, WireValue::Unsigned(old.saturating_add(1))));
            }
        }
        for name in [
            "depot_unbunching_last_departure",
            "depot_unbunching_next_departure",
        ] {
            edits.push(common(id, name, WireValue::Unsigned(0)));
        }
        edits.push(common(id, "round_trip_time", WireValue::Signed(0)));
    }
    Ok(edits)
}
pub(super) fn common(id: u32, name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: id,
        path: vec![
            PathElement::Field("roadveh".into()),
            PathElement::Index(0),
            PathElement::Field("common".into()),
            PathElement::Index(0),
            PathElement::Field(name.into()),
        ],
        value,
    }
}
