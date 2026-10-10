use super::RuntimeError;
use ottd_save::{TableChunk, TableRecord, TableSchema, WireValue, world::World};
pub(super) fn table(world: &World, chunk: [u8; 4]) -> Result<&TableChunk, RuntimeError> {
    world
        .tables()
        .get(&chunk)
        .ok_or(RuntimeError::Invalid("order table"))
}
#[derive(Debug, Clone, Copy)]
pub(super) struct Row<'a> {
    pub schema: &'a TableSchema,
    pub record: &'a TableRecord,
}
impl<'a> Row<'a> {
    pub(super) fn field(self, name: &str) -> Result<&'a WireValue, RuntimeError> {
        self.schema
            .fields()
            .iter()
            .zip(self.record.values())
            .find_map(|(f, v)| (f.name() == name).then_some(v))
            .ok_or(RuntimeError::Invalid("order field"))
    }
    pub(super) fn number(self, name: &str) -> Result<u64, RuntimeError> {
        match self.field(name)? {
            WireValue::Unsigned(n) => Ok(*n),
            _ => Err(RuntimeError::Invalid("order unsigned field")),
        }
    }
    pub(super) fn signed(self, name: &str) -> Result<i64, RuntimeError> {
        match self.field(name)? {
            WireValue::Signed(value) => Ok(*value),
            _ => Err(RuntimeError::Invalid("order signed field")),
        }
    }
    pub(super) fn children(
        self,
        name: &str,
    ) -> Result<(&'a TableSchema, &'a [TableRecord]), RuntimeError> {
        let schema = self
            .schema
            .fields()
            .iter()
            .find(|f| f.name() == name)
            .and_then(ottd_save::FieldSchema::child)
            .ok_or(RuntimeError::Invalid("order child schema"))?;
        match self.field(name)? {
            WireValue::Structs(rows) => Ok((schema, rows)),
            _ => Err(RuntimeError::Invalid("order child rows")),
        }
    }
}
pub(super) fn row(world: &World, chunk: [u8; 4], id: u32) -> Result<Row<'_>, RuntimeError> {
    let t = table(world, chunk)?;
    Ok(Row {
        schema: t.schema(),
        record: t
            .records()
            .get(&id)
            .ok_or(RuntimeError::Invalid("order record"))?,
    })
}
#[derive(Debug, Clone, Copy)]
pub(in crate::runtime) enum OrderReader<'a> {
    Committed(&'a World),
    Candidate(ottd_save::world::CandidateView<'a>),
}
impl<'a> OrderReader<'a> {
    pub(super) fn rows(self, chunk: [u8; 4]) -> Result<Vec<(u32, Row<'a>)>, RuntimeError> {
        match self {
            Self::Committed(world) => {
                let t = table(world, chunk)?;
                Ok(t.records()
                    .iter()
                    .map(|(id, record)| {
                        (
                            *id,
                            Row {
                                schema: t.schema(),
                                record,
                            },
                        )
                    })
                    .collect())
            }
            Self::Candidate(view) => {
                let t = view
                    .table(chunk)
                    .ok_or(RuntimeError::Invalid("order candidate table"))?;
                Ok(t.records()
                    .map(|(id, record)| {
                        (
                            id,
                            Row {
                                schema: t.schema(),
                                record,
                            },
                        )
                    })
                    .collect())
            }
        }
    }
    pub(super) fn tile(self, index: u32) -> Result<ottd_save::TileState, RuntimeError> {
        match self {
            Self::Committed(world) => world
                .map()
                .tiles()
                .get(usize::try_from(index).map_err(|_| RuntimeError::Invalid("order tile"))?)
                .cloned()
                .ok_or(RuntimeError::Invalid("order tile")),
            Self::Candidate(view) => Ok(view.tile(index)?),
        }
    }
}
pub(super) fn vehicle(row: Row<'_>) -> Result<Option<(&'static str, Row<'_>)>, RuntimeError> {
    let variant = match row.number("type")? {
        0 => "train",
        1 => "roadveh",
        2 => "ship",
        3 => "aircraft",
        4 | 5 => return Ok(None),
        _ => return Err(RuntimeError::Invalid("order vehicle type")),
    };
    let (schema, rows) = row.children(variant)?;
    let [record] = rows else {
        return Err(RuntimeError::Invalid("vehicle variant"));
    };
    let (schema, rows) = (Row { schema, record }).children("common")?;
    let [record] = rows else {
        return Err(RuntimeError::Invalid("vehicle common"));
    };
    Ok(Some((variant, Row { schema, record })))
}

pub(super) fn consist(world: &World, id: u32) -> Result<Row<'_>, RuntimeError> {
    vehicle(row(world, *b"VEHS", id)?)?
        .map(|(_, row)| row)
        .ok_or(RuntimeError::Unsupported(
            "effect/disaster backup lifecycle",
        ))
}
