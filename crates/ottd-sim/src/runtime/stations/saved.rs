#![expect(
    clippy::redundant_pub_crate,
    reason = "station transaction adapters are shared internally with commands"
)]
use super::RuntimeError;
use crate::world_access::row_field;
use ottd_save::{TableRecord, TableSchema, WireValue, world::World};

#[derive(Clone, Copy)]
pub(crate) struct StationRow<'a> {
    pub schema: &'a TableSchema,
    pub record: &'a TableRecord,
}
impl<'a> StationRow<'a> {
    pub(crate) fn number(self, name: &str) -> Result<u64, RuntimeError> {
        match row_field(self.schema, self.record, name)
            .map_err(|_| RuntimeError::Invalid("station field"))?
        {
            WireValue::Unsigned(value) => Ok(*value),
            _ => Err(RuntimeError::Invalid("station unsigned field")),
        }
    }
    pub(crate) fn child(self, name: &str) -> Result<Self, RuntimeError> {
        let (schema, rows) = self.rows(name)?;
        if rows.len() != 1 {
            return Err(RuntimeError::Invalid("station singleton"));
        }
        Ok(Self {
            schema,
            record: rows.first().ok_or(RuntimeError::Invalid("station row"))?,
        })
    }
    pub(crate) fn rows(
        self,
        name: &str,
    ) -> Result<(&'a TableSchema, &'a [TableRecord]), RuntimeError> {
        let schema = self
            .schema
            .fields()
            .iter()
            .find(|f| f.name() == name)
            .and_then(ottd_save::FieldSchema::child)
            .ok_or(RuntimeError::Invalid("station child schema"))?;
        match row_field(self.schema, self.record, name)
            .map_err(|_| RuntimeError::Invalid("station child field"))?
        {
            WireValue::Structs(rows) => Ok((schema, rows)),
            _ => Err(RuntimeError::Invalid("station child rows")),
        }
    }
}
pub(crate) fn row(world: &World, chunk: [u8; 4], id: u32) -> Result<StationRow<'_>, RuntimeError> {
    let table = world
        .tables()
        .get(&chunk)
        .ok_or(RuntimeError::Invalid("station table"))?;
    Ok(StationRow {
        schema: table.schema(),
        record: table
            .records()
            .get(&id)
            .ok_or(RuntimeError::Invalid("station record"))?,
    })
}

#[derive(Clone, Copy)]
pub(super) enum StationRead<'a> {
    Committed(&'a World),
    Candidate(ottd_save::world::CandidateView<'a>),
}
impl<'a> StationRead<'a> {
    pub(super) fn row(self, chunk: [u8; 4], id: u32) -> Result<StationRow<'a>, RuntimeError> {
        match self {
            Self::Committed(world) => row(world, chunk, id),
            Self::Candidate(view) => {
                let table = view
                    .table(chunk)
                    .ok_or(RuntimeError::Invalid("station candidate table"))?;
                Ok(StationRow {
                    schema: table.schema(),
                    record: table
                        .record(id)
                        .ok_or(RuntimeError::Invalid("station candidate record"))?,
                })
            }
        }
    }
    pub(super) fn tile(self, index: u32) -> Result<ottd_save::TileState, RuntimeError> {
        match self {
            Self::Committed(world) => world
                .map()
                .tiles()
                .get(
                    usize::try_from(index)
                        .map_err(|_| RuntimeError::Invalid("station tile index"))?,
                )
                .cloned()
                .ok_or(RuntimeError::Invalid("station tile index")),
            Self::Candidate(view) => Ok(view.tile(index)?),
        }
    }
}
