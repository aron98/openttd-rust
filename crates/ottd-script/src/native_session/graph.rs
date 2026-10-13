//! Source-ordered construction of the initial graph, with an unpublished
//! init guard recording completed milestones for deterministic rollback.
use super::{error::NativeContextError, primary, roots::SharedRoots};
use crate::{
    Realm,
    heap::{Handle, Object, Stored, Unpublished},
    native_closure::NativeClosure,
    table,
    table::scalar::Scalar,
};

/// Capacity request for the metamethod map (`MT_LAST-1`), rounded by the
/// physical table to its native node count of thirty-two.
const METAMETHOD_CAPACITY: usize = 17;

/// Construction guard: owns the completed prefix and every live zero-reference
/// token; its drop rolls back an incomplete graph in reverse milestone order
/// and drains recorded orphans directly, never calling collection.
pub(super) struct Builder {
    realm: Realm,
    pub(super) roots: SharedRoots,
    unpublished: Vec<Unpublished>,
    complete: bool,
}
impl Builder {
    pub(super) fn new(realm: &Realm) -> Self {
        Self {
            realm: realm.clone(),
            roots: SharedRoots::empty(),
            unpublished: Vec::new(),
            complete: false,
        }
    }
    /// Record a live zero-reference construction token for rollback drains.
    pub(super) fn track(&mut self, unpublished: Unpublished) {
        self.unpublished.push(unpublished);
    }
    pub(super) const fn realm(&self) -> &Realm {
        &self.realm
    }
    /// Build the complete initial graph; the guard completes on success.
    pub(super) fn build(
        mut self,
        initial_slots: usize,
    ) -> Result<(SharedRoots, super::context::ContextRecord), NativeContextError> {
        self.shared_prefix()?;
        let record = primary::build(&mut self, initial_slots)?;
        self.complete = true;
        let roots = std::mem::replace(&mut self.roots, SharedRoots::empty());
        Ok((roots, record))
    }
    fn shared_prefix(&mut self) -> Result<(), NativeContextError> {
        let realm = self.realm().clone();
        // _metamethodsmap = SQTable::Create(this, MT_LAST-1), published at its
        // field assignment before any entry exists.
        let map = realm.heap.unpublished(
            &realm,
            Object::Table(table::Data::new(METAMETHOD_CAPACITY)?),
        )?;
        let map = map.publish_shared();
        self.roots.meta = Some(map);
        // Fifteen system strings, then eighteen metamethod entries whose keys
        // the vector and the map slot each separately own.
        self.roots
            .system_strings
            .try_reserve(super::roots::SYSTEM_STRINGS.len())
            .map_err(|_| NativeContextError::Allocation)?;
        for name in super::roots::SYSTEM_STRINGS {
            self.roots.system_strings.push(realm.string(name));
        }
        self.roots
            .metamethods
            .try_reserve(super::roots::METAMETHODS.len())
            .map_err(|_| NativeContextError::Allocation)?;
        self.insert_metamethods()?;
        // _constructoridx, then registry and consts.
        self.roots.constructor = Some(realm.string(b"constructor"));
        self.roots.registry = Some(self.empty_table()?);
        self.roots.consts = Some(self.empty_table()?);
        // Ten default tables, each immediately populated in ledger order.
        self.roots
            .defaults
            .try_reserve(super::roots::CONSTRUCTION_ORDER.len())
            .map_err(|_| NativeContextError::Allocation)?;
        for registry in super::roots::CONSTRUCTION_ORDER {
            let table = self.default_table(registry.registrations())?;
            self.roots.defaults.push((registry, table));
        }
        Ok(())
    }
    #[expect(
        clippy::unreachable,
        reason = "The metamethod map token was allocated with table storage"
    )]
    fn insert_metamethods(&mut self) -> Result<(), NativeContextError> {
        let realm = self.realm().clone();
        let meta = self
            .roots
            .meta
            .clone()
            .ok_or(NativeContextError::Allocation)?;
        let handle = Handle::from_edge(&meta, &realm);
        let result: Result<(), crate::VmError> = (|| {
            for (index, name) in super::roots::METAMETHODS.iter().enumerate() {
                let key = realm.string(name);
                self.roots.metamethods.push(key.clone());
                let scalar = Scalar::String(key);
                let hash = scalar.hash()?;
                let value = Stored::Scalar(Scalar::Integer(
                    i64::try_from(index).map_err(|_| crate::VmError::Allocation)?,
                ));
                handle.mutate(|object| match object {
                    Object::Table(data) => data.nodes.insert(scalar, hash, value),
                    Object::Array(_)
                    | Object::NativeClosure(_)
                    | Object::ScriptClosure(_)
                    | Object::Context(_) => unreachable!(),
                })?;
            }
            Ok(())
        })();
        drop(handle);
        result.map_err(NativeContextError::from)
    }
    fn empty_table(&self) -> Result<crate::heap::Edge, NativeContextError> {
        let realm = self.realm().clone();
        let table = realm
            .heap
            .unpublished(&realm, Object::Table(table::Data::new(0)?))?;
        Ok(table.publish_shared())
    }
    /// `CreateDefaultDelegate`: a fresh zero-reference table, then its fresh
    /// closures in ledger order; the shared field publishes after the entries.
    #[expect(
        clippy::unreachable,
        reason = "The default table token was allocated with table storage"
    )]
    fn default_table(
        &mut self,
        rows: &'static [crate::NativeRegistration],
    ) -> Result<crate::heap::Edge, NativeContextError> {
        let realm = self.realm().clone();
        let table = realm
            .heap
            .unpublished(&realm, Object::Table(table::Data::new(0)?))?;
        self.track(table.share());
        for row in rows {
            let closure = NativeClosure::unpublished(&realm, row)?;
            let name = realm.string(row.name());
            let scalar = Scalar::String(name);
            let hash = scalar.hash()?;
            let value = Stored::NativeClosure(closure.stored());
            table.mutate(|object| match object {
                Object::Table(data) => data.nodes.insert(scalar.clone(), hash, value),
                Object::Array(_)
                | Object::NativeClosure(_)
                | Object::ScriptClosure(_)
                | Object::Context(_) => unreachable!(),
            })?;
        }
        Ok(table.publish_shared())
    }
}
impl Drop for Builder {
    /// Reverse-milestone rollback for an incomplete construction; native has
    /// no failure oracle, so this only guarantees zero leaked owners.
    fn drop(&mut self) {
        if self.complete {
            return;
        }
        // Primary teardown precedes shared-prefix rollback.
        if let Some(primary) = self.roots.primary.take() {
            drop(primary);
        }
        while let Some((_, table)) = self.roots.defaults.pop() {
            drop(table);
        }
        for field in [
            &mut self.roots.consts,
            &mut self.roots.meta,
            &mut self.roots.registry,
        ] {
            if let Some(edge) = field.take() {
                drop(edge);
            }
        }
        self.roots.constructor = None;
        self.roots.types.clear();
        while let Some(string) = self.roots.system_strings.pop() {
            drop(string);
        }
        self.roots.metamethods.clear();
        // Drain the recorded zero-reference orphan prefix in reverse order.
        while let Some(unpublished) = self.unpublished.pop() {
            self.realm.heap.release_orphan(&unpublished);
        }
        self.realm.heap.drain_releases();
    }
}
