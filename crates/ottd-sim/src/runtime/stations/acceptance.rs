//! Native vanilla house acceptance over the actual terminal catchment union.
use super::{RoadStationGeometry, RuntimeError};
use crate::runtime::transport::cargo::read::Read;
use ottd_save::{
    WireValue,
    world::{PathElement, WorldEdit},
};

pub(crate) fn for_world(
    world: &ottd_save::world::World,
    geometry: &RoadStationGeometry,
    station: u32,
    existing: bool,
) -> Result<Vec<WorldEdit>, RuntimeError> {
    calculate(Read::World(world), geometry)?.edits(Read::World(world), station, existing)
}

pub(in crate::runtime) struct Acceptance {
    pub amounts: [u32; 64],
    pub always: u64,
}

pub(in crate::runtime) fn calculate(
    view: Read<'_>,
    geometry: &RoadStationGeometry,
) -> Result<Acceptance, RuntimeError> {
    let mut result = Acceptance {
        amounts: [0; 64],
        always: 0,
    };
    for index in &geometry.catchment {
        let tile = view.tile(*index)?;
        match tile.tile_type() >> 4 {
            0 | 1 | 2 | 4 | 6 | 7 | 9 => {}
            5 if matches!((tile.m6() >> 3) & 15, 2 | 3) => {}
            3 => {
                if view.number(*b"PATS", 0, "game_creation.landscape")? != 0
                    || !view.ids(*b"NGRF")?.is_empty()
                {
                    return Err(RuntimeError::Unsupported(
                        "custom/non-temperate house acceptance",
                    ));
                }
                let acceptance = super::acceptance_data::HOUSES
                    .get(usize::from(tile.m8() & 0x0fff))
                    .ok_or(RuntimeError::Unsupported("custom house acceptance ID"))?;
                for (cargo, amount) in acceptance {
                    if *cargo == 255 || *amount == 0 {
                        continue;
                    }
                    let total = result
                        .amounts
                        .get_mut(usize::from(*cargo))
                        .ok_or(RuntimeError::Invalid("acceptance cargo ID"))?;
                    *total = total.wrapping_add(u32::from(*amount));
                    result.always |= 1_u64 << cargo;
                }
            }
            _ => {
                return Err(RuntimeError::Unsupported(
                    "station accepting tile procedure",
                ));
            }
        }
    }
    for (cargo, amount) in result.amounts.iter_mut().enumerate() {
        if (cargo == 0 && geometry.bus.is_none()) || (cargo != 0 && geometry.truck.is_none()) {
            *amount = 0;
        }
    }
    Ok(result)
}

impl Acceptance {
    pub(in crate::runtime) fn edits(
        &self,
        view: Read<'_>,
        station: u32,
        existing: bool,
    ) -> Result<Vec<WorldEdit>, RuntimeError> {
        let mut edits = Vec::new();
        let saved = if existing {
            Some(
                view.row(*b"STNN", station)?
                    .child("normal")?
                    .rows("goods")?,
            )
        } else {
            None
        };
        for (cargo, amount) in self.amounts.iter().enumerate() {
            let mut status = 0;
            if let Some((schema, rows)) = saved {
                let row = super::saved::StationRow {
                    schema,
                    record: rows
                        .get(cargo)
                        .ok_or(RuntimeError::Invalid("station goods cardinality"))?,
                };
                status = row.number("status")?;
                let graph = row.number("link_graph")?;
                if graph != 65535 {
                    let graph = u32::try_from(graph)
                        .map_err(|_| RuntimeError::Invalid("acceptance graph"))?;
                    let node = usize::try_from(row.number("node")?)
                        .map_err(|_| RuntimeError::Invalid("acceptance node"))?;
                    let (schema, nodes) = view.row(*b"LGRP", graph)?.rows("nodes")?;
                    let node_row = super::saved::StationRow {
                        schema,
                        record: nodes
                            .get(node)
                            .ok_or(RuntimeError::Invalid("acceptance node"))?,
                    };
                    if node_row.number("station")? != u64::from(station) {
                        return Err(RuntimeError::Invalid("acceptance node membership"));
                    }
                    edits.push(WorldEdit::Field {
                        chunk: *b"LGRP",
                        record: graph,
                        path: vec![
                            PathElement::Field("nodes".into()),
                            PathElement::Index(node),
                            PathElement::Field("demand".into()),
                        ],
                        value: WireValue::Unsigned(u64::from(amount / 8)),
                    });
                }
            }
            edits.push(WorldEdit::Field {
                chunk: *b"STNN",
                record: station,
                path: vec![
                    PathElement::Field("normal".into()),
                    PathElement::Index(0),
                    PathElement::Field("goods".into()),
                    PathElement::Index(cargo),
                    PathElement::Field("status".into()),
                ],
                value: WireValue::Unsigned((status & !1) | u64::from(*amount >= 8)),
            });
        }
        edits.push(WorldEdit::Field {
            chunk: *b"STNN",
            record: station,
            path: vec![
                PathElement::Field("normal".into()),
                PathElement::Index(0),
                PathElement::Field("always_accepted".into()),
            ],
            value: WireValue::Unsigned(self.always),
        });
        Ok(edits)
    }
}
