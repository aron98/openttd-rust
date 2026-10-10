mod order_delta;
mod stage;
pub use order_delta::OrderDurationDelta;
#[cfg(test)]
mod tests;
mod view;
use super::{
    DerivedState, MapState, TableRecord, TileState, WireValue, World, WorldEdit, WorldError,
    derived, edit, invalid, limits, references, scripts, semantics,
};
use crate::snapshot::{SnapshotMetadata, world as snapshot};
use std::collections::BTreeMap;
pub use view::{CandidateMap, CandidateTable, CandidateView};
type Records = BTreeMap<[u8; 4], BTreeMap<u32, Option<TableRecord>>>;
type Tiles = BTreeMap<u32, TileState>;

/// Sparse candidate edits; the borrowed world remains unchanged until commit.
#[derive(Debug)]
pub struct WorldTransaction<'world> {
    world: &'world mut World,
    records: Records,
    tiles: Tiles,
    poisoned: bool,
}

/// Validated candidate plus ready structural projections; dropping discards it.
#[derive(Debug)]
pub struct PreparedWorldTransaction<'world> {
    transaction: WorldTransaction<'world>,
    metadata: SnapshotMetadata,
    map: Option<MapState>,
    derived: DerivedState,
}

impl World {
    /// Begin an isolated candidate; dropping or forgetting it leaves this world unchanged.
    pub const fn transaction(&mut self) -> WorldTransaction<'_> {
        WorldTransaction {
            world: self,
            records: BTreeMap::new(),
            tiles: BTreeMap::new(),
            poisoned: false,
        }
    }
}
impl<'world> WorldTransaction<'world> {
    /// Borrow the current candidate, including earlier staged writes.
    pub const fn view(&self) -> CandidateView<'_> {
        CandidateView {
            world: self.world,
            records: &self.records,
            tiles: &self.tiles,
        }
    }
    /// Number of distinct records and tiles retained by this candidate.
    pub fn touched(&self) -> (usize, usize) {
        (
            self.records.values().map(BTreeMap::len).sum(),
            self.tiles.len(),
        )
    }
    /// Stage an edit; an error poisons this candidate and prevents publication.
    /// # Errors
    /// Rejects invalid edit targets and incompatible field/list operations.
    pub fn apply(&mut self, edit: WorldEdit) -> Result<(), WorldError> {
        if self.poisoned {
            return Err(invalid("transaction", "candidate is poisoned"));
        }
        let result = self.stage(edit);
        self.poisoned = result.is_err();
        result
    }
    /// Validate the complete candidate once as a batch, without encoding a save.
    /// # Errors
    /// Rejects poisoned edits and every existing wire, snapshot or world invariant.
    pub fn prepare(mut self) -> Result<PreparedWorldTransaction<'world>, WorldError> {
        if self.poisoned {
            return Err(invalid("transaction", "candidate is poisoned"));
        }
        let candidate = {
            let installed = Installed::new(self.world, &mut self.records, &mut self.tiles);
            validate(installed.world)?
        };
        Ok(PreparedWorldTransaction {
            transaction: self,
            metadata: candidate.0,
            map: candidate.1,
            derived: candidate.2,
        })
    }
}
impl PreparedWorldTransaction<'_> {
    /// Borrow validated candidate records and tiles without exposing mutation.
    pub const fn view(&self) -> CandidateView<'_> {
        self.transaction.view()
    }
    /// Candidate map reuses original storage unless dimensions changed.
    pub fn map(&self) -> CandidateMap<'_> {
        CandidateMap {
            base: self
                .map
                .as_ref()
                .unwrap_or_else(|| self.transaction.world.map()),
            tiles: &self.transaction.tiles,
        }
    }
    /// Structural indexes reconstructed for this candidate.
    pub const fn derived(&self) -> &DerivedState {
        &self.derived
    }
    /// Publish only prevalidated values; no callbacks or fallible work run here.
    pub fn commit(mut self) {
        swap_records(self.transaction.world, &mut self.transaction.records);
        swap_tiles(self.transaction.world, &mut self.transaction.tiles);
        for (chunk, table) in &mut self.transaction.world.tables {
            if self.transaction.records.contains_key(chunk) {
                table.normalize_slots();
            }
        }
        self.transaction
            .world
            .snapshot
            .replace_metadata(self.metadata);
        if let Some(map) = self.map {
            self.transaction.world.snapshot.replace_map(map);
        }
        self.transaction.world.derived = self.derived;
    }
}

struct Installed<'world, 'edits> {
    world: &'world mut World,
    records: &'edits mut Records,
    tiles: &'edits mut Tiles,
}
impl<'world, 'edits> Installed<'world, 'edits> {
    fn new(
        world: &'world mut World,
        records: &'edits mut Records,
        tiles: &'edits mut Tiles,
    ) -> Self {
        swap_records(world, records);
        swap_tiles(world, tiles);
        Self {
            world,
            records,
            tiles,
        }
    }
}
impl Drop for Installed<'_, '_> {
    fn drop(&mut self) {
        swap_records(self.world, self.records);
        swap_tiles(self.world, self.tiles);
    }
}
fn swap_records(world: &mut World, edits: &mut Records) {
    for (chunk, table) in &mut world.tables {
        if let Some(rows) = edits.get_mut(chunk) {
            for (id, value) in rows {
                *value = match value.take() {
                    Some(value) => table.records_mut().insert(*id, value),
                    None => table.records_mut().remove(id),
                };
            }
        }
    }
}
fn swap_tiles(world: &mut World, edits: &mut Tiles) {
    for (index, tile) in edits {
        let index = usize::try_from(*index).unwrap_or(usize::MAX);
        for (chunk, plane) in &mut world.planes {
            let (bytes, width) = match chunk {
                b"MAPT" => ([tile.tile_type(), 0], 1),
                b"MAPH" => ([tile.height(), 0], 1),
                b"MAPO" => ([tile.m1(), 0], 1),
                b"MAP2" => (tile.m2().to_be_bytes(), 2),
                b"M3LO" => ([tile.m3(), 0], 1),
                b"M3HI" => ([tile.m4(), 0], 1),
                b"MAP5" => ([tile.m5(), 0], 1),
                b"MAPE" => ([tile.m6(), 0], 1),
                b"MAP7" => ([tile.m7(), 0], 1),
                b"MAP8" => (tile.m8().to_be_bytes(), 2),
                _ => continue,
            };
            for (target, byte) in plane
                .iter_mut()
                .skip(index.saturating_mul(width))
                .zip(bytes.into_iter().take(width))
            {
                *target = byte;
            }
        }
        world.snapshot.swap_tile(index, tile);
    }
}
fn validate(
    world: &World,
) -> Result<(SnapshotMetadata, Option<MapState>, DerivedState), WorldError> {
    wire_len(world, crate::DEFAULT_MAX_BYTES)?;
    limits::validate(&world.tables)?;
    let map = snapshot::map(&world.tables, &world.planes, world.map())?;
    let metadata = snapshot::metadata(&world.tables)?;
    semantics::validate(&world.tables)?;
    references::validate(&world.tables, map.as_ref().unwrap_or_else(|| world.map()))?;
    scripts::validate(&world.tables)?;
    let derived = derived::restore(&world.tables)?;
    Ok((metadata, map, derived))
}
fn wire_len(world: &World, limit: usize) -> Result<usize, WorldError> {
    let mut length = 12usize;
    for table in world.tables.values() {
        length = length
            .saturating_add(5)
            .saturating_add(table.validated_wire_len()?);
    }
    for plane in world.planes.values() {
        length = length.saturating_add(8).saturating_add(plane.len());
    }
    if length > limit {
        return Err(crate::Error::SizeLimit(limit).into());
    }
    Ok(length)
}
