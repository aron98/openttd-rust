//! The materialized idle session factory and its shared state.
use super::{
    close, collector::NativeCollector, context::ContextSlot, error::NativeContextError,
    graph::Builder, ref_identity::RawSlot, ref_table::RefTable, roots::SharedRoots,
};
use crate::{Realm, heap::Stored};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Lifecycle phases of the shared state; collection has no valid native
/// capability while closing or closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    /// Fully constructed and idle.
    Live,
    /// The source close sequence is releasing owners.
    Closing,
    /// Every shared owner was released.
    Closed,
}
/// Weakly shareable core: owning fields, context registry, physical `RefTable`
/// and phase; no field retains the realm across the realm's own weak hook.
#[derive(Debug)]
pub(crate) struct SharedState {
    pub(super) realm: Realm,
    pub(super) phase: Cell<Phase>,
    pub(super) generation: Cell<u64>,
    pub(super) collecting: Cell<bool>,
    pub(super) roots: RefCell<Option<SharedRoots>>,
    pub(super) refs: RefCell<Option<RefTable>>,
    pub(super) contexts: RefCell<Vec<ContextSlot>>,
}
impl SharedState {
    pub(super) fn phase(&self) -> Phase {
        self.phase.get()
    }
    pub(super) fn live_generation(&self) -> u64 {
        self.generation.get()
    }
    /// Explicit host retention of one refcounted value through the physical
    /// `RefTable` (`sqapi.cpp:155 sq_addref`); the caller owns the passed
    /// reference and drops it after this call. Non-refcounted values are
    /// ignored exactly like the source guard.
    /// # Errors
    /// Returns `Allocation` when the identity cannot be read or a full table
    /// cannot double its storage.
    pub(super) fn retain_stored(&self, value: Stored) -> Result<(), NativeContextError> {
        let Some(key) = RawSlot::of_stored(&value, &self.realm) else {
            return Ok(());
        };
        let key = key?;
        let mut refs = self.refs.borrow_mut();
        let Some(table) = refs.as_mut() else {
            return Err(NativeContextError::ClosedContext);
        };
        let result = table.add_ref(value, key);
        drop(refs);
        self.realm.heap.drain_releases();
        result
    }
    /// Explicit host release of one refcounted value (`sqapi.cpp:165
    /// sq_release`); the final release drops the node's owning reference.
    /// # Errors
    /// Returns `Allocation` on lookup failure; absence reports `false`.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "explicit host release; the atom's receipt witnesses read it"
        )
    )]
    pub(super) fn release_stored(&self, value: &Stored) -> Result<bool, NativeContextError> {
        let Some(key) = RawSlot::of_stored(value, &self.realm) else {
            return Ok(true);
        };
        let key = key?;
        let mut refs = self.refs.borrow_mut();
        let Some(table) = refs.as_mut() else {
            return Err(NativeContextError::ClosedContext);
        };
        let result = table.release(key);
        drop(refs);
        self.realm.heap.drain_releases();
        result
    }
}
/// A materialized native session: the complete initial graph of the pinned
/// native build, with idle collection and source-ordered close.
///
/// It grants no compiler, execution, active-print or `RefTable` retention
/// capability; those requests are refused before any state mutation until
/// their atoms land.
#[derive(Debug)]
pub struct NativeSession {
    pub(super) realm: Realm,
    pub(super) shared: Rc<SharedState>,
}
/// Capabilities a materialized idle graph intentionally cannot grant yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeCapability {
    /// Compiling source inside the session realm.
    Compiler,
    /// Borrowing the primary context for a frame or executing bytecode.
    Execution,
    /// Installing an active print sink that observes or collects mid-print.
    ActivePrint,
    /// Retaining a host object in the physical `RefTable`.
    NonEmptyRefTable,
}
impl NativeSession {
    /// Materialize a fresh realm and its complete initial graph, exactly as
    /// the pinned native build constructs it: four `RefTable` slots, string
    /// infrastructure, the metamethod map, fifteen system strings and the
    /// constructor string, registry and consts, the ten populated default
    /// tables, the primary context and its root table.
    ///
    /// A zero-sized initial stack is rejected: the native release build could
    /// not even complete its base-registration pushes with it.
    /// # Errors
    /// Returns `InvalidInitialSlots` for a zero-sized stack, or `Allocation`
    /// when a checked construction reservation fails.
    pub fn open(initial_slots: usize) -> Result<Self, NativeContextError> {
        if initial_slots == 0 {
            return Err(NativeContextError::InvalidInitialSlots);
        }
        let realm = Realm::new();
        // C++ member construction runs before the shared constructor body, so
        // the physical RefTable with its four slots exists before anything else.
        let refs = RefTable::new()?;
        let builder = Builder::new(&realm);
        let (roots, record) = builder.build(initial_slots)?;
        let shared = Rc::new(SharedState {
            realm: realm.clone(),
            phase: Cell::new(Phase::Live),
            generation: Cell::new(1),
            collecting: Cell::new(false),
            roots: RefCell::new(Some(roots)),
            refs: RefCell::new(Some(refs)),
            contexts: RefCell::new(vec![ContextSlot::from_record(record)]),
        });
        realm.heap.install_native_session(Rc::downgrade(&shared));
        Ok(Self { realm, shared })
    }
    /// The materialized realm; ordinary collection on it dispatches to the
    /// native root enumerator of this session while it stays live.
    pub const fn realm(&self) -> &Realm {
        &self.realm
    }
    /// A weak native collector bound to this session's generation; it adds no
    /// graph owner and never keeps the session logically open.
    pub fn collector(&self) -> NativeCollector {
        NativeCollector::new(&self.shared)
    }
    /// Request one execution capability; the idle graph refuses every one of
    /// them before touching session state, without inventing script errors.
    /// # Errors
    /// Always returns `IncompleteNativeGraph` in this atom.
    pub const fn request_capability(
        &self,
        capability: NativeCapability,
    ) -> Result<(), NativeContextError> {
        let _ = capability;
        Err(NativeContextError::IncompleteNativeGraph)
    }
    /// Allocate a friend context and retain it explicitly in the physical
    /// `RefTable` (`sq_newthread` plus `sq_addref`): real friend allocation,
    /// `Init` from the primary, `Push` onto the primary stack, `AddRef`,
    /// then `Pop`, so the token's host retention is the friend's sole owner
    /// (design §81). The token's drop performs the matching release.
    /// # Errors
    /// Returns `InvalidInitialSlots` for a zero-sized stack, or `Allocation`
    /// when a checked construction reservation fails.
    pub fn retain_friend(
        &mut self,
        initial_slots: usize,
    ) -> Result<super::friend::RetainedNativeFriend, NativeContextError> {
        super::friend::retain_friend(&self.shared, initial_slots)
    }
    /// Repeat one explicit `RefTable` host retention of an existing friend
    /// (`sq_addref` on the same object): the node's handle count grows and
    /// the returned token drops one release.
    /// # Errors
    /// Fails with the token validation results of `friend`.
    pub fn retain(
        &mut self,
        token: &super::friend::RetainedNativeFriend,
    ) -> Result<super::friend::RetainedNativeFriend, NativeContextError> {
        super::friend::retain_again(&self.shared, token)
    }
    /// Borrow one retained friend for a scoped view bound to both the
    /// session and the token, validating realm and generation; the friend's
    /// stable private payload moves into a storage lease for the borrow and
    /// reattaches allocation-free on drop, and collection during the view
    /// cannot finalize the host-rooted payload.
    /// # Errors
    /// Returns `ForeignContext`, `StaleContext` or `ClosedContext` for a
    /// foreign, stale or closed token.
    pub fn friend<'a>(
        &'a mut self,
        token: &'a super::friend::RetainedNativeFriend,
    ) -> Result<super::friend_view::NativeFriendView<'a>, NativeContextError> {
        super::friend::friend(&self.shared, token)
    }
    /// Consume the session and run the exact source close sequence
    /// (`sq_close` plus `~SQSharedState`), releasing every owner in order.
    pub fn close(self) {}
}
impl Drop for NativeSession {
    fn drop(&mut self) {
        close::close(&self.shared);
    }
}
