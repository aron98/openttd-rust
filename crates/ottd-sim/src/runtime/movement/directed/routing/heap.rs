//! Native `CBinaryHeapT` ordering: strict estimate comparison, left child on ties.
use super::{WorldTickError, unsupported};
#[derive(Default)]
pub(super) struct Heap(Vec<(usize, i64)>);
impl Heap {
    pub(super) fn first(&self) -> Option<usize> {
        self.0.first().map(|&(id, _)| id)
    }
    pub(super) fn push(&mut self, value: (usize, i64)) {
        self.0.push(value);
        let mut gap = self.0.len().wrapping_sub(1);
        while gap != 0 {
            let parent = gap.wrapping_sub(1) / 2;
            if self.0.get(parent).is_none_or(|item| item.1 <= value.1) {
                break;
            }
            self.0.swap(gap, parent);
            gap = parent;
        }
    }
    pub(super) fn remove(&mut self, id: usize) -> Result<(), WorldTickError> {
        let mut gap = self
            .0
            .iter()
            .position(|item| item.0 == id)
            .ok_or_else(|| unsupported("road_yapf", "missing open heap node"))?;
        self.0.swap_remove(gap);
        let Some(value) = self.0.get(gap).copied() else {
            return Ok(());
        };
        while gap != 0 {
            let parent = gap.wrapping_sub(1) / 2;
            if self.0.get(parent).is_none_or(|item| item.1 <= value.1) {
                break;
            }
            self.0.swap(gap, parent);
            gap = parent;
        }
        loop {
            let left = gap.wrapping_mul(2).wrapping_add(1);
            let Some(left_value) = self.0.get(left) else {
                break;
            };
            let right = left.wrapping_add(1);
            let child = if self.0.get(right).is_some_and(|item| item.1 < left_value.1) {
                right
            } else {
                left
            };
            if self.0.get(child).is_none_or(|item| item.1 >= value.1) {
                break;
            }
            self.0.swap(gap, child);
            gap = child;
        }
        Ok(())
    }
}
