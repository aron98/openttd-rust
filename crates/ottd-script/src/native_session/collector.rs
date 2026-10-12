//! Weak native collection with the source root-enqueue order.
use super::{error::NativeContextError, session::SharedState};
use std::rc::Weak;

/// A weak collection capability bound to one session generation; it may
/// transiently upgrade memory custody but never adds a graph owner or keeps
/// the session logically open.
#[derive(Clone, Debug)]
pub struct NativeCollector {
    state: Weak<SharedState>,
    generation: u64,
}
impl NativeCollector {
    pub(super) fn new(state: &std::rc::Rc<SharedState>) -> Self {
        Self {
            state: std::rc::Rc::downgrade(state),
            generation: state.live_generation(),
        }
    }
    /// Collect with `SQSharedState::CollectGarbage`'s root-enqueue order:
    /// the primary context, the physical `RefTable` objects, registry, consts,
    /// metamethod map and the ten default delegates in generator/thread-
    /// before-closure order. Marking happens at enqueue, the chain is rebuilt
    /// from the LIFO pop order and unmarked nodes finalize in chain order.
    /// # Errors
    /// Fails with `ClosedContext`/`StaleContext` when the session is gone or
    /// reused, or `CollectionReentry` on a reentrant collection.
    pub fn collect(&self) -> Result<usize, NativeContextError> {
        let state = self
            .state
            .upgrade()
            .ok_or(NativeContextError::ClosedContext)?;
        if state.phase() != super::session::Phase::Live {
            return Err(NativeContextError::ClosedContext);
        }
        if state.live_generation() != self.generation {
            return Err(NativeContextError::StaleContext);
        }
        if state.collecting.replace(true) {
            return Err(NativeContextError::CollectionReentry);
        }
        let collected = collect_shared_state(&state);
        state.collecting.set(false);
        Ok(collected)
    }
}
/// Source-ordered native collection entry, also used by the materialized
/// realm's `collect_cycles` dispatch; it is inert while closing or closed.
pub(crate) fn collect_shared_state(state: &SharedState) -> usize {
    if state.phase() != super::session::Phase::Live {
        return 0;
    }
    let roots = state.roots.borrow();
    let Some(roots) = roots.as_ref() else {
        return 0;
    };
    let edges = roots.collect_order();
    // Physical `RefTable` nodes enqueue between the primary context and the
    // registry; the idle session's table holds none, so no root edge follows it.
    state.realm.heap.collect_native(&edges)
}
