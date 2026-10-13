//! Native path annotation arithmetic and parent ownership without raw pointers.
use super::{super::job_executor::WorkerError, model::Path};
type Result<T> = std::result::Result<T, WorkerError>;

#[derive(Clone, Copy)]
pub(super) enum Metric {
    Distance,
    Capacity,
}
impl Path {
    pub(super) const fn new(node: u16, source: bool) -> Self {
        Self {
            node,
            origin: if source { node } else { u16::MAX },
            parent: None,
            distance: if source { 0 } else { u32::MAX },
            capacity: if source { u32::MAX } else { 0 },
            free_capacity: if source { i32::MAX } else { i32::MIN },
            flow: 0,
            children: 0,
        }
    }
    pub(super) fn rank(&self, metric: Metric) -> (i64, i32) {
        match metric {
            Metric::Distance => (i64::from(self.distance), i32::from(self.node)),
            Metric::Capacity => (
                i64::from(ratio(self.free_capacity, self.capacity)).wrapping_neg(),
                i32::from(self.node).wrapping_neg(),
            ),
        }
    }
    pub(super) fn better(&self, base: &Self, leg: &Leg, metric: Metric) -> bool {
        match metric {
            Metric::Distance => {
                if base.distance == u32::MAX {
                    return false;
                }
                if self.distance == u32::MAX {
                    return true;
                }
                if (leg.free > 0 && base.free_capacity > 0) != (self.free_capacity > 0) {
                    return self.free_capacity <= 0;
                }
                base.distance.wrapping_add(leg.distance) < self.distance
            }
            Metric::Capacity => {
                let next = ratio(
                    base.free_capacity.min(leg.free),
                    base.capacity.min(leg.capacity),
                );
                let current = ratio(self.free_capacity, self.capacity);
                if next == current {
                    base.distance != u32::MAX
                        && base.distance.wrapping_add(leg.distance) < self.distance
                } else {
                    next > current
                }
            }
        }
    }
}
pub(super) fn ratio(free: i32, total: u32) -> i32 {
    let minimum = (i32::MIN + 1) / 16;
    let maximum = (i32::MAX - 1) / 16;
    let free = if free < minimum {
        minimum
    } else if free > maximum {
        maximum
    } else {
        free
    };
    // C++ divides the signed product by unsigned total, then converts the quotient to int.
    let product = u32::from_ne_bytes(free.wrapping_mul(16).to_ne_bytes());
    let divisor = std::num::NonZeroU32::new(total).unwrap_or(std::num::NonZeroU32::MIN);
    i32::from_ne_bytes((product / divisor).to_ne_bytes())
}
pub(super) struct Leg {
    pub capacity: u32,
    pub free: i32,
    pub distance: u32,
}
pub(super) fn get(paths: &[Option<Path>], id: u16) -> Result<&Path> {
    paths
        .get(usize::from(id))
        .and_then(Option::as_ref)
        .ok_or(WorkerError::InvalidInput)
}
fn get_mut(paths: &mut [Option<Path>], id: u16) -> Result<&mut Path> {
    paths
        .get_mut(usize::from(id))
        .and_then(Option::as_mut)
        .ok_or(WorkerError::InvalidInput)
}
fn detach(paths: &mut [Option<Path>], id: u16) -> Result<()> {
    if let Some(parent) = get_mut(paths, id)?.parent.take() {
        let parent = get_mut(paths, parent)?;
        parent.children = parent
            .children
            .checked_sub(1)
            .ok_or(WorkerError::InvalidInput)?;
    }
    Ok(())
}
pub(super) fn fork(paths: &mut [Option<Path>], edge: (u16, u16), leg: &Leg) -> Result<()> {
    let (from, to) = edge;
    let base = get(paths, from)?.clone();
    if get(paths, to)?.parent != Some(from) {
        detach(paths, to)?;
        get_mut(paths, to)?.parent = Some(from);
        get_mut(paths, from)?.children = base.children.wrapping_add(1);
    }
    let path = get_mut(paths, to)?;
    path.capacity = base.capacity.min(leg.capacity);
    path.free_capacity = base.free_capacity.min(leg.free);
    path.distance = base.distance.wrapping_add(leg.distance);
    path.origin = base.origin;
    Ok(())
}
pub(super) fn cleanup(paths: &mut Vec<Option<Path>>, source: u16) -> Result<()> {
    for index in 0..paths.len() {
        let id = u16::try_from(index).map_err(|_| WorkerError::InvalidInput)?;
        if id == source || paths.get(index).is_none_or(Option::is_none) {
            continue;
        }
        if get(paths, id)?.parent == Some(source) {
            detach(paths, id)?;
        }
        let mut current = Some(id);
        while let Some(id) = current {
            if id == source || get(paths, id)?.flow != 0 {
                break;
            }
            current = get(paths, id)?.parent;
            detach(paths, id)?;
            if get(paths, id)?.children == 0 {
                *paths
                    .get_mut(usize::from(id))
                    .ok_or(WorkerError::InvalidInput)? = None;
            }
        }
    }
    paths.clear();
    Ok(())
}
