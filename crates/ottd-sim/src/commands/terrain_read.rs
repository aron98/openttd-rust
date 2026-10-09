use super::CommandError;
use crate::world_access::{WorldAccessError, row_field};
use ottd_save::{
    TileState, WireValue,
    world::{CandidateView, World},
};

#[derive(Clone, Copy)]
pub(super) struct MapSize {
    width: u32,
    height: u32,
}
impl MapSize {
    pub(super) const fn from_world(world: &World) -> Self {
        Self {
            width: world.map().width(),
            height: world.map().height(),
        }
    }
    pub(super) const fn width(self) -> u32 {
        self.width
    }
    pub(super) const fn height(self) -> u32 {
        self.height
    }
    pub(super) fn count(self) -> Result<u32, CommandError> {
        self.width
            .checked_mul(self.height)
            .ok_or(CommandError::Overflow("map size"))
    }
}
#[derive(Clone, Copy)]
pub(super) enum TerrainRead<'a> {
    Committed(&'a World),
    Candidate {
        view: CandidateView<'a>,
        size: MapSize,
    },
}
impl<'a> TerrainRead<'a> {
    pub(super) const fn size(self) -> MapSize {
        match self {
            Self::Committed(world) => MapSize::from_world(world),
            Self::Candidate { size, .. } => size,
        }
    }
    pub(super) fn tile(self, tile: u32) -> Result<TileState, CommandError> {
        match self {
            Self::Committed(world) => super::landscape::tile_at(world, tile).cloned(),
            Self::Candidate { view, .. } => Ok(view.tile(tile)?),
        }
    }
    pub(super) fn field(
        self,
        chunk: [u8; 4],
        id: u32,
        name: &str,
    ) -> Result<&'a WireValue, WorldAccessError> {
        match self {
            Self::Committed(world) => crate::world_access::field(world, &chunk, id, name),
            Self::Candidate { view, .. } => {
                let table = view
                    .table(chunk)
                    .ok_or_else(|| WorldAccessError(format!("{chunk:?}")))?;
                let row = table
                    .record(id)
                    .ok_or_else(|| WorldAccessError(format!("{chunk:?}/{id}")))?;
                row_field(table.schema(), row, name)
            }
        }
    }
    pub(super) fn unsigned(self, chunk: [u8; 4], id: u32, name: &str) -> Result<u64, CommandError> {
        match self.field(chunk, id, name)? {
            WireValue::Unsigned(value) => Ok(*value),
            WireValue::Signed(value) => {
                u64::try_from(*value).map_err(|_| WorldAccessError(name.into()).into())
            }
            _ => Err(WorldAccessError(name.into()).into()),
        }
    }
    pub(super) fn signed(self, chunk: [u8; 4], id: u32, name: &str) -> Result<i64, CommandError> {
        match self.field(chunk, id, name)? {
            WireValue::Signed(value) => Ok(*value),
            _ => Err(WorldAccessError(name.into()).into()),
        }
    }
    pub(super) fn base_height(self, index: u32) -> Result<u8, CommandError> {
        let size = self.size();
        let width =
            std::num::NonZeroU32::new(size.width).ok_or(CommandError::Overflow("map width"))?;
        let (x, y) = (index % width, index / width);
        let x2 = x.saturating_add(1).min(size.width.saturating_sub(1));
        let y2 = y.saturating_add(1).min(size.height.saturating_sub(1));
        let height = |x: u32, y: u32| -> Result<u8, CommandError> {
            let tile = y
                .checked_mul(size.width)
                .and_then(|n| n.checked_add(x))
                .ok_or(CommandError::Overflow("corner index"))?;
            Ok(self.tile(tile)?.height())
        };
        ottd_core::terrain::slope_from_corners([
            height(x, y)?,
            height(x2, y)?,
            height(x, y2)?,
            height(x2, y2)?,
        ])
        .map(|(_, base)| base)
        .map_err(|_| CommandError::Unsupported("invalid tunnel scan geometry"))
    }
}
