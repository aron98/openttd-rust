//! Test-only lookup helpers over one idle session snapshot.
use super::SessionSnapshot;
use crate::heap::{
    ObjectId,
    observation::{Counts, HeapSnapshot, Identity, ObjectKind},
};

/// Count live entries of one object kind in a snapshot.
pub(in crate::native_session) fn kind_count(snapshot: &HeapSnapshot, kind: ObjectKind) -> usize {
    snapshot
        .entries
        .iter()
        .filter(|entry| entry.kind == Some(kind) && entry.life.is_some())
        .count()
}
/// Look up the heap counts of one identity.
pub(in crate::native_session) fn counts(
    snapshot: &HeapSnapshot,
    identity: Identity,
) -> Option<Counts> {
    snapshot
        .entries
        .iter()
        .find(|entry| entry.identity == identity)
        .and_then(|entry: &crate::heap::observation::EntrySnapshot| entry.life)
}
/// Map a chain to labeled positions, like the receipt's chain line.
pub(in crate::native_session) fn chain_positions(snapshot: &SessionSnapshot) -> Vec<(usize, char)> {
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
pub(in crate::native_session) fn label_identity(
    snapshot: &SessionSnapshot,
    label: char,
) -> Option<Identity> {
    snapshot
        .labels
        .iter()
        .find(|(candidate, _)| *candidate == label)
        .map(|(_, identity)| *identity)
}
/// Borrow the materialized root table as a public table value for content
/// assertions; the test-only owner is released when the value drops.
pub(in crate::native_session) fn observation_root_table(
    session: &super::super::session::NativeSession,
) -> Option<crate::Table> {
    let shared = &session.shared;
    let roots = shared.roots.borrow();
    let roots = roots.as_ref()?;
    let primary = roots.primary.as_ref()?;
    let handle = crate::heap::Handle::from_edge(primary, &shared.realm);
    let edge = handle.read(|object| match object {
        crate::heap::Object::Context(payload) => {
            match &payload.persistent()[super::super::payload::ROOT] {
                crate::heap::Stored::Table(edge) => Some(edge.clone()),
                _ => None,
            }
        }
        crate::heap::Object::Array(_)
        | crate::heap::Object::NativeClosure(_)
        | crate::heap::Object::ScriptClosure(_)
        | crate::heap::Object::Table(_) => None,
    });
    drop(handle);
    edge.map(|edge| crate::Table(crate::heap::Handle::from_edge(&edge, &shared.realm)))
}

/// Labeled chain positions for the session's known labels plus test-local
/// identities, filtered to the receipt's labels. The receipt's own labels
/// shadow the shared-field labels of the same letter, exactly like the
/// observer's per-mode labeling.
pub(in crate::native_session) fn labeled_positions(
    snapshot: &SessionSnapshot,
    extra: &[(char, ObjectId)],
    wanted: &[char],
) -> Vec<(usize, char)> {
    let mut labels = extra
        .iter()
        .map(|(label, id)| {
            (
                *label,
                Identity {
                    index: id.index,
                    generation: id.generation,
                },
            )
        })
        .collect::<Vec<_>>();
    labels.extend(
        snapshot
            .labels
            .iter()
            .filter(|(label, _)| !extra.iter().any(|(extra, _)| extra == label))
            .map(|(label, identity)| (*label, *identity)),
    );
    snapshot
        .heap
        .chain
        .iter()
        .enumerate()
        .filter_map(|(index, identity)| {
            labels
                .iter()
                .find(|(_, candidate)| candidate == identity)
                .map(|(label, _)| (index, *label))
        })
        .filter(|(_, label)| wanted.contains(label))
        .collect()
}
/// The label characters of one receipt chain occurrence, in order.
pub(in crate::native_session) fn receipt_labels(raw: &str, occurrence: usize) -> Vec<char> {
    super::super::receipts::chain_positions(raw, occurrence)
        .iter()
        .map(|(_, label)| *label)
        .collect()
}
