use super::{
    CommandError,
    terrain_read::{MapSize, TerrainRead},
};
use crate::content::Prices;
use ottd_save::world::{World, WorldEdit, WorldTransaction};

pub(super) struct TerrainState<'world, 'prices> {
    transaction: WorldTransaction<'world>,
    size: MapSize,
    pub(super) prices: &'prices Prices,
}
impl<'world, 'prices> TerrainState<'world, 'prices> {
    pub(super) const fn new(world: &'world mut World, prices: &'prices Prices) -> Self {
        let size = MapSize::from_world(world);
        Self {
            transaction: world.transaction(),
            size,
            prices,
        }
    }
    pub(super) const fn read(&self) -> TerrainRead<'_> {
        TerrainRead::Candidate {
            view: self.transaction.view(),
            size: self.size,
        }
    }
    pub(super) fn apply(&mut self, edits: Vec<WorldEdit>) -> Result<(), CommandError> {
        for edit in edits {
            self.transaction.apply(edit)?;
        }
        Ok(())
    }
    pub(super) fn commit(self) -> Result<(), CommandError> {
        self.transaction.prepare()?.commit();
        Ok(())
    }
}
