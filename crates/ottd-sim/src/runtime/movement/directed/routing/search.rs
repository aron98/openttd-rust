//! Native YAPF open/closed transitions with stable node identity on replacement.
use super::{Costs, Node, WorldTickError, heap::Heap, network::Position, unsupported};
use std::collections::BTreeMap;
#[derive(Default)]
struct Search {
    nodes: Vec<Node>,
    heap: Heap,
    open: BTreeMap<(u32, u8), usize>,
    closed: BTreeMap<(u32, u8), usize>,
    intermediate: Option<usize>,
}
pub(super) struct Path {
    pub track: u8,
    pub cache: Vec<Position>,
    pub found: bool,
}
impl Search {
    fn node(&self, id: usize) -> Result<Node, WorldTickError> {
        self.nodes
            .get(id)
            .copied()
            .ok_or_else(|| unsupported("road_yapf", "node arena"))
    }
    fn insert(&mut self, node: Node, track_intermediate: bool) -> Result<(), WorldTickError> {
        let key = node.start.key()?;
        let intermediate = track_intermediate
            && match self.intermediate {
                None => true,
                Some(id) => {
                    let old = self.node(id)?;
                    old.estimate.wrapping_sub(old.cost) > node.estimate.wrapping_sub(node.cost)
                }
            };
        let id = if let Some(&id) = self.open.get(&key) {
            if node.estimate >= self.node(id)?.estimate {
                return Ok(());
            }
            self.heap.remove(id)?;
            *self
                .nodes
                .get_mut(id)
                .ok_or_else(|| unsupported("road_yapf", "open node identity"))? = node;
            id
        } else {
            if let Some(&id) = self.closed.get(&key) {
                if node.estimate < self.node(id)?.estimate {
                    return Err(unsupported("road_yapf", "inconsistent closed estimate"));
                }
                return Ok(());
            }
            let id = self.nodes.len();
            self.nodes.push(node);
            self.open.insert(key, id);
            id
        };
        self.heap.push((id, node.estimate));
        if intermediate {
            self.intermediate = Some(id);
        }
        Ok(())
    }
    fn path(&self, id: usize, found: bool) -> Result<Path, WorldTickError> {
        let mut node = self.node(id)?;
        let mut steps = 0_usize;
        while let Some(parent) = node.parent {
            steps = steps.wrapping_add(1);
            node = self.node(parent)?;
        }
        node = self.node(id)?;
        let mut cache = Vec::with_capacity(8);
        while let Some(parent) = node.parent {
            steps = steps.wrapping_sub(1);
            if node.choice && steps < 8 {
                cache.push(node.start);
            }
            node = self.node(parent)?;
        }
        Ok(Path {
            track: node.start.track,
            cache,
            found,
        })
    }
}
pub(super) fn find(
    costs: &Costs<'_, '_>,
    origin: (u32, u16),
    limit: usize,
) -> Result<Option<Path>, WorldTickError> {
    let mut search = Search::default();
    for track in 0_u8..14 {
        if origin.1 & (1 << track) == 0 {
            continue;
        }
        let start = Position {
            tile: origin.0,
            track,
        };
        search.insert(
            Node {
                start,
                end: start,
                parent: None,
                cost: 0,
                estimate: 0,
                choice: false,
            },
            false,
        )?;
    }
    while let Some(id) = search.heap.first() {
        let parent = search.node(id)?;
        if costs.target(parent.end)? {
            return search.path(id, true).map(Some);
        }
        if let Some((tile, bits)) = costs.network.follow(parent.end)? {
            for track in 0_u8..14 {
                if bits & (1 << track) == 0 {
                    continue;
                }
                let start = Position { tile, track };
                let mut node = Node {
                    start,
                    end: start,
                    parent: Some(id),
                    cost: parent.cost,
                    estimate: 0,
                    choice: bits.count_ones() > 1,
                };
                if costs.evaluate(&mut node)? {
                    if node.estimate < parent.estimate {
                        return Err(unsupported("road_yapf", "non-monotone estimate"));
                    }
                    search.insert(node, limit > 0)?;
                }
            }
        }
        if limit != 0 && search.closed.len() >= limit {
            break;
        }
        search.heap.remove(id)?;
        let key = parent.start.key()?;
        search.open.remove(&key);
        search.closed.insert(key, id);
    }
    search
        .intermediate
        .map(|id| search.path(id, false))
        .transpose()
}
