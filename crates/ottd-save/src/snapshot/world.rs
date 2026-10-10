use super::{ScriptRandomState, SnapshotMetadata, date, invalid, map, schema, table};
use crate::{MapState, TableChunk, world::WorldError};
use std::collections::BTreeMap;
type Tables = BTreeMap<[u8; 4], TableChunk>;

fn encoded(tables: &Tables, id: [u8; 4]) -> Result<crate::Chunk, WorldError> {
    Ok(tables
        .get(&id)
        .ok_or_else(|| invalid("missing snapshot table"))?
        .encode()?)
}
pub(crate) fn metadata(tables: &Tables) -> Result<SnapshotMetadata, WorldError> {
    let date = date::from_chunk(&encoded(tables, *b"DATE")?)?;
    let settings = table::single(&encoded(tables, *b"PATS")?, schema::SETTINGS)?;
    let random = table::decode(&encoded(tables, *b"SRND")?, schema::RANDOM)?;
    let random = random
        .into_iter()
        .map(|(owner, fields)| {
            Ok(ScriptRandomState::new(
                owner,
                [
                    table::unsigned(&fields, "state[0]")?,
                    table::unsigned(&fields, "state[1]")?,
                ],
            ))
        })
        .collect::<Result<Vec<_>, super::SnapshotError>>()?;
    Ok(SnapshotMetadata::new(date, settings, random)?)
}
pub(crate) fn map(
    tables: &Tables,
    planes: &BTreeMap<[u8; 4], Vec<u8>>,
    current: &MapState,
) -> Result<Option<MapState>, WorldError> {
    let fields = table::single(&encoded(tables, *b"MAPS")?, schema::MAP)?;
    let width = table::unsigned(&fields, "dim_x")?;
    let height = table::unsigned(&fields, "dim_y")?;
    if (width, height) == (current.width(), current.height()) {
        return Ok(None);
    }
    Ok(Some(map::from_planes(width, height, |id| {
        planes
            .get(&id)
            .map(Vec::as_slice)
            .ok_or_else(|| invalid("missing map plane"))
    })?))
}
