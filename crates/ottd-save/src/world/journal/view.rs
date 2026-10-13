use super::{Records, Tiles};
use crate::{
    MapState, TableChunk, TableRecord, TableSchema, TileState,
    world::{World, WorldError, invalid},
};
use std::collections::BTreeMap;

/// Borrowed candidate records and tiles; committed derived caches are not exposed.
#[derive(Debug, Clone, Copy)]
pub struct CandidateView<'a> {
    pub(super) world: &'a World,
    pub(super) records: &'a Records,
    pub(super) tiles: &'a Tiles,
}
impl<'a> CandidateView<'a> {
    /// Read a table with sparse candidate overrides applied.
    pub fn table(self, chunk: [u8; 4]) -> Option<CandidateTable<'a>> {
        self.world.tables.get(&chunk).map(|base| CandidateTable {
            base,
            rows: self.records.get(&chunk),
        })
    }
    /// Read the latest candidate tile.
    /// # Errors
    /// Rejects an out-of-map index.
    pub fn tile(self, index: u32) -> Result<TileState, WorldError> {
        CandidateMap {
            base: self.world.map(),
            tiles: self.tiles,
        }
        .tile(index)
        .cloned()
        .ok_or_else(|| invalid("tile", "index out of range"))
    }
}
/// A candidate pool table without copying untouched records.
#[derive(Debug, Clone, Copy)]
pub struct CandidateTable<'a> {
    base: &'a TableChunk,
    rows: Option<&'a BTreeMap<u32, Option<TableRecord>>>,
}
impl<'a> CandidateTable<'a> {
    /// Pinned immutable schema shared with the committed table.
    pub const fn schema(self) -> &'a TableSchema {
        self.base.schema()
    }
    /// Borrow the latest record, or None after deletion.
    pub fn record(self, id: u32) -> Option<&'a TableRecord> {
        self.rows
            .and_then(|rows| rows.get(&id))
            .map_or_else(|| self.base.records().get(&id), Option::as_ref)
    }
    /// Ascending native identities with insertions and deletions applied.
    pub fn records(self) -> impl Iterator<Item = (u32, &'a TableRecord)> {
        let mut base = self.base.records().iter().peekable();
        let mut edits = self
            .rows
            .into_iter()
            .flat_map(|rows| rows.iter())
            .peekable();
        std::iter::from_fn(move || {
            loop {
                match (
                    base.peek().map(|(id, _)| **id),
                    edits.peek().map(|(id, _)| **id),
                ) {
                    (None, None) => return None,
                    (Some(id), Some(edit)) if id < edit => {
                        return base.next().map(|(id, row)| (*id, row));
                    }
                    (Some(_), None) => return base.next().map(|(id, row)| (*id, row)),
                    (id, Some(edit)) => {
                        if id == Some(edit) {
                            base.next();
                        }
                        if let Some((id, Some(row))) = edits.next() {
                            return Some((*id, row));
                        }
                    }
                }
            }
        })
    }
}
/// Candidate map borrowing unchanged tiles and overlaying only touched tiles.
#[derive(Debug, Clone, Copy)]
pub struct CandidateMap<'a> {
    pub(super) base: &'a MapState,
    pub(super) tiles: &'a Tiles,
}
impl<'a> CandidateMap<'a> {
    /// Width in tiles.
    pub const fn width(self) -> u32 {
        self.base.width()
    }
    /// Height in tiles.
    pub const fn height(self) -> u32 {
        self.base.height()
    }
    /// One candidate tile by linear index.
    pub fn tile(self, index: u32) -> Option<&'a TileState> {
        self.tiles.get(&index).or_else(|| {
            usize::try_from(index)
                .ok()
                .and_then(|index| self.base.tiles().get(index))
        })
    }
}
