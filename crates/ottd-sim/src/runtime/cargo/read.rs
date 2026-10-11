//! One borrowed view for committed restoration and candidate cache publication.
use crate::runtime::{
    RuntimeError,
    stations::saved::{self, StationRow},
};
use ottd_save::{
    TileState,
    world::{CandidateView, World},
};

#[derive(Clone, Copy)]
pub(in crate::runtime) enum Read<'a> {
    World(&'a World),
    Candidate(CandidateView<'a>),
}
impl<'a> Read<'a> {
    pub(in crate::runtime) fn row(
        self,
        chunk: [u8; 4],
        id: u32,
    ) -> Result<StationRow<'a>, RuntimeError> {
        match self {
            Self::World(world) => saved::row(world, chunk, id),
            Self::Candidate(view) => {
                let table = view
                    .table(chunk)
                    .ok_or(RuntimeError::Invalid("cargo candidate table"))?;
                Ok(StationRow {
                    schema: table.schema(),
                    record: table
                        .record(id)
                        .ok_or(RuntimeError::Invalid("cargo candidate row"))?,
                })
            }
        }
    }
    pub(in crate::runtime) fn number(
        self,
        chunk: [u8; 4],
        id: u32,
        name: &str,
    ) -> Result<u64, RuntimeError> {
        let row = self.row(chunk, id)?;
        match crate::world_access::row_field(row.schema, row.record, name)
            .map_err(|_| RuntimeError::Invalid("cargo numeric field"))?
        {
            ottd_save::WireValue::Unsigned(value) => Ok(*value),
            ottd_save::WireValue::Signed(value) => {
                u64::try_from(*value).map_err(|_| RuntimeError::Invalid("cargo numeric range"))
            }
            _ => Err(RuntimeError::Invalid("cargo numeric shape")),
        }
    }

    pub(in crate::runtime) fn ids(self, chunk: [u8; 4]) -> Result<Vec<u32>, RuntimeError> {
        match self {
            Self::World(world) => Ok(world
                .tables()
                .get(&chunk)
                .ok_or(RuntimeError::Invalid("cargo table"))?
                .records()
                .keys()
                .copied()
                .collect()),
            Self::Candidate(view) => Ok(view
                .table(chunk)
                .ok_or(RuntimeError::Invalid("cargo candidate table"))?
                .records()
                .map(|(id, _)| id)
                .collect()),
        }
    }
    pub(in crate::runtime) fn tile(self, index: u32) -> Result<TileState, RuntimeError> {
        match self {
            Self::World(world) => world
                .map()
                .tiles()
                .get(usize::try_from(index).map_err(|_| RuntimeError::Invalid("cargo tile"))?)
                .cloned()
                .ok_or(RuntimeError::Invalid("cargo tile")),
            Self::Candidate(view) => Ok(view.tile(index)?),
        }
    }
}
