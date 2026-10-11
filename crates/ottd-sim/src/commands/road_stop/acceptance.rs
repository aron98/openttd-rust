use crate::{
    commands::{CommandError, landscape},
    runtime::{RoadStationGeometry, stations::saved},
};
use ottd_save::{
    WireValue,
    world::{PathElement, World, WorldEdit},
};

pub(super) fn zero_acceptance(
    world: &World,
    geometry: &RoadStationGeometry,
    station: Option<u32>,
) -> Result<Vec<WorldEdit>, CommandError> {
    for tile in &geometry.catchment {
        let source = landscape::tile_at(world, *tile)?;
        match source.tile_type() >> 4 {
            0 | 1 | 2 | 4 | 6 | 7 | 9 => {}
            5 if matches!((source.m6() >> 3) & 15, 2 | 3) => {}
            _ => {
                return Err(CommandError::Unsupported(
                    "road station accepting catchment",
                ));
            }
        }
    }
    let mut edits = Vec::new();
    if let Some(id) = station {
        let normal = saved::row(world, *b"STNN", id)?.child("normal")?;
        let (schema, rows) = normal.rows("goods")?;
        if rows.len() != 64 {
            return Err(CommandError::Unsupported("station goods cardinality"));
        }
        for (index, record) in rows.iter().enumerate() {
            let row = saved::StationRow { schema, record };
            if row.number("link_graph")? != 65535 {
                return Err(CommandError::Unsupported("station link graph demand"));
            }
            edits.push(WorldEdit::Field {
                chunk: *b"STNN",
                record: id,
                path: vec![
                    PathElement::Field("normal".into()),
                    PathElement::Index(0),
                    PathElement::Field("goods".into()),
                    PathElement::Index(index),
                    PathElement::Field("status".into()),
                ],
                value: WireValue::Unsigned(row.number("status")? & !1),
            });
        }
        edits.push(super::effects::normal(
            id,
            "always_accepted",
            WireValue::Unsigned(0),
        ));
    }
    Ok(edits)
}

pub(super) fn terminal_station(
    world: &World,
    station: u32,
    geometry: &RoadStationGeometry,
) -> Result<(), CommandError> {
    let row = saved::row(world, *b"STNN", station)?;
    for name in ["speclist", "roadstopspeclist", "roadstoptiledata"] {
        if !row.rows(name)?.1.is_empty() {
            return Err(CommandError::Unsupported(
                "joining custom station specifications",
            ));
        }
    }
    let area = geometry
        .station
        .ok_or(CommandError::Unsupported("joining deleted station"))?;
    for y in area.top..=area.bottom {
        for x in area.left..=area.right {
            let index = y
                .checked_mul(world.map().width())
                .and_then(|n| n.checked_add(x))
                .ok_or(CommandError::Overflow("station area tile"))?;
            let tile = landscape::tile_at(world, index)?;
            if tile.tile_type() >> 4 == 5 && u32::from(tile.m2()) == station && tile.m5() >= 4 {
                return Err(CommandError::Unsupported(
                    "joining drive-through road stops",
                ));
            }
        }
    }
    Ok(())
}
