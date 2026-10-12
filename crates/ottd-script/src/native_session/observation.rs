//! Test-only scalar copies of the idle session state; no observation adds an
//! owner or mutates the graph.
use super::{
    context::{ContextId, ContextPhase, RootMembership},
    payload,
    session::SharedState,
};
use crate::heap::observation::{Counts, EntrySnapshot, HeapSnapshot, Identity, ObjectKind};
use crate::heap::{Handle, Object, Stored};

/// One idle snapshot: heap state, the source-labeled identities, the primary
/// context scalars, the persistent-field memberships and the `RefTable` state.
#[derive(Debug)]
pub(super) struct SessionSnapshot {
    pub(super) heap: HeapSnapshot,
    pub(super) labels: Vec<(char, Identity)>,
    pub(super) vm: Option<VmLine>,
    pub(super) fields: [(&'static str, RootMembership); 5],
    pub(super) ref_slots: u64,
    pub(super) ref_used: u64,
    pub(super) ref_free: Vec<Option<usize>>,
    pub(super) context: (ContextId, ContextPhase, u64),
    pub(super) memberships: ([RootMembership; 5], RootMembership),
    /// Source-default scalars of the four allocated call records.
    pub(super) call_records: Vec<super::scalars::CallScalars>,
}
/// Scalar context counters in the receipt's `roots vm` line shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct VmLine {
    pub(super) references: usize,
    pub(super) stack_size: usize,
    pub(super) top: i64,
    pub(super) stackbase: i64,
    pub(super) vargs_size: usize,
    pub(super) callsstacksize: i64,
    pub(super) etraps: i64,
    pub(super) ops: i64,
}
impl SharedState {
    /// Label the source-known identities exactly like the native observer:
    /// the primary context `V`, root table `O`, registry `R`, consts `C`,
    /// metamethod map `M` and the ten default tables `T A S N F G H K I W`.
    fn labeled(&self, roots: &super::roots::SharedRoots) -> Vec<(char, Identity)> {
        let mut labels = Vec::new();
        let primary_edge = roots.primary.as_ref().expect("live primary");
        let primary: Identity = primary_edge.identity().into();
        labels.push(('V', primary));
        let handle = Handle::from_edge(primary_edge, &self.realm);
        let root = handle.read(|object| match object {
            Object::Context(payload) => match payload.persistent()[payload::ROOT] {
                Stored::Table(ref edge) => Some(Identity::from(edge.identity())),
                _ => None,
            },
            Object::Array(_)
            | Object::NativeClosure(_)
            | Object::ScriptClosure(_)
            | Object::Table(_) => None,
        });
        drop(handle);
        if let Some(root) = root {
            labels.push(('O', root));
        }
        if let Some(edge) = &roots.registry {
            labels.push(('R', edge.identity().into()));
        }
        if let Some(edge) = &roots.consts {
            labels.push(('C', edge.identity().into()));
        }
        if let Some(edge) = &roots.meta {
            labels.push(('M', edge.identity().into()));
        }
        for (registry, edge) in &roots.defaults {
            if let Some(label) = match registry.name() {
                "table" => Some('T'),
                "array" => Some('A'),
                "string" => Some('S'),
                "number" => Some('N'),
                "closure" => Some('F'),
                "generator" => Some('G'),
                "thread" => Some('H'),
                "class" => Some('K'),
                "instance" => Some('I'),
                "weakref" => Some('W'),
                _ => None,
            } {
                labels.push((label, edge.identity().into()));
            }
        }
        labels
    }
    /// Copy the scalar idle state of the whole session.
    pub(super) fn snapshot(&self) -> SessionSnapshot {
        let heap = self.realm.heap.snapshot();
        let roots = self.roots.borrow();
        let labels = roots
            .as_ref()
            .map(|roots| self.labeled(roots))
            .unwrap_or_default();
        let registry = self
            .contexts
            .borrow()
            .iter()
            .flatten()
            .map(|record| {
                (
                    record.id,
                    record.life.phase(),
                    record.life.references(),
                    // The registry's scalar membership is the authority; the
                    // shared `_root_vm` field itself targets this context.
                    (record.membership, RootMembership::Context(record.id)),
                )
            })
            .next();
        let context = registry.map_or(
            (
                ContextId {
                    index: 0,
                    generation: 0,
                },
                ContextPhase::Retired,
                0,
                ([RootMembership::Null; 5], RootMembership::Null),
            ),
            |record| record,
        );
        let mut vm = None;
        let mut call_records = Vec::new();
        let fields = ["error", "handler", "debug", "root", "temp"]
            .into_iter()
            .zip(context.3.0)
            .collect::<Vec<_>>()
            .try_into()
            .expect("five persistent fields");
        if let Some(roots) = roots.as_ref() {
            let primary_edge = roots.primary.as_ref().expect("live primary");
            let references = self
                .realm
                .heap
                .entry_references(primary_edge.identity().into());
            let handle = Handle::from_edge(primary_edge, &self.realm);
            handle.read(|object| match object {
                Object::Context(payload) => {
                    let scalars = payload.scalars();
                    call_records.extend(payload.call_records());
                    vm = Some(VmLine {
                        references,
                        stack_size: scalars.stack_size,
                        top: scalars.top,
                        stackbase: scalars.stackbase,
                        vargs_size: scalars.vargs_size,
                        callsstacksize: scalars.callsstacksize,
                        etraps: scalars.etraps,
                        ops: scalars.ops_till_suspend,
                    });
                }
                Object::Array(_)
                | Object::NativeClosure(_)
                | Object::ScriptClosure(_)
                | Object::Table(_) => {}
            });
        }
        let (ref_slots, ref_used, ref_free) = self
            .refs
            .borrow()
            .as_ref()
            .map_or((0, 0, Vec::new()), |table| {
                (table.numofslots(), table.slotused(), table.free_chain())
            });
        SessionSnapshot {
            heap,
            labels,
            vm,
            fields,
            ref_slots,
            ref_used,
            ref_free,
            context: (context.0, context.1, context.2),
            memberships: context.3,
            call_records,
        }
    }
}
/// Count live entries of one object kind in a snapshot.
pub(super) fn kind_count(snapshot: &HeapSnapshot, kind: ObjectKind) -> usize {
    snapshot
        .entries
        .iter()
        .filter(|entry| entry.kind == Some(kind) && entry.life.is_some())
        .count()
}
/// Look up the heap counts of one identity.
pub(super) fn counts(snapshot: &HeapSnapshot, identity: Identity) -> Option<Counts> {
    snapshot
        .entries
        .iter()
        .find(|entry| entry.identity == identity)
        .and_then(|entry: &EntrySnapshot| entry.life)
}
/// Map a chain to labeled positions, like the receipt's chain line.
pub(super) fn chain_positions(snapshot: &SessionSnapshot) -> Vec<(usize, char)> {
    snapshot
        .heap
        .chain
        .iter()
        .enumerate()
        .filter_map(|(index, identity)| {
            snapshot
                .labels
                .iter()
                .find(|(_, candidate)| candidate == identity)
                .map(|(label, _)| (index, *label))
        })
        .collect()
}
/// The identity a snapshot label carries.
pub(super) fn label_identity(snapshot: &SessionSnapshot, label: char) -> Option<Identity> {
    snapshot
        .labels
        .iter()
        .find(|(candidate, _)| *candidate == label)
        .map(|(_, identity)| *identity)
}
/// Borrow the materialized root table as a public table value for content
/// assertions; the test-only owner is released when the value drops.
pub(super) fn observation_root_table(
    session: &super::session::NativeSession,
) -> Option<crate::Table> {
    let shared = &session.shared;
    let roots = shared.roots.borrow();
    let roots = roots.as_ref()?;
    let primary = roots.primary.as_ref()?;
    let handle = Handle::from_edge(primary, &shared.realm);
    let edge = handle.read(|object| match object {
        Object::Context(payload) => match &payload.persistent()[payload::ROOT] {
            Stored::Table(edge) => Some(edge.clone()),
            _ => None,
        },
        Object::Array(_)
        | Object::NativeClosure(_)
        | Object::ScriptClosure(_)
        | Object::Table(_) => None,
    });
    drop(handle);
    edge.map(|edge| crate::Table(Handle::from_edge(&edge, &shared.realm)))
}
