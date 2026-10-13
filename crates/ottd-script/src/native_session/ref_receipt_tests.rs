//! Genuine original-backed comparisons of the physical `RefTable` retention
//! and the retained friend against the native receipts; these run only with
//! the capture directory provided.
use super::observation;
use super::receipts;
use super::ref_identity::RawSlot;
use crate::heap::{Object, ObjectId, Stored};
use crate::{NativeSession, heap::observation::Identity, table, table::scalar::Scalar};

/// One fresh empty table retained like `sq_addref`; dropping the
/// construction owner leaves the `RefTable` node as the sole owner. The raw
/// identity key of the table comes back like the observer's raw handle.
fn identity(id: ObjectId) -> Identity {
    Identity {
        index: id.index,
        generation: id.generation,
    }
}
fn retain_table(session: &NativeSession) -> (ObjectId, RawSlot) {
    let realm = session.realm();
    let unpublished = realm
        .heap
        .unpublished(realm, Object::Table(table::Data::new(0).expect("table")))
        .expect("table allocation");
    let edge = unpublished.publish_shared();
    let id = edge.identity();
    let stored = Stored::Table(edge);
    let key = RawSlot::of_stored(&stored, realm)
        .expect("refcounted table")
        .expect("identity");
    session
        .shared
        .retain_stored(stored)
        .expect("host retention");
    (id, key)
}
/// Clone the owning reference of one retained raw identity, like the
/// observer's raw `HSQOBJECT` handle for a repeated `sq_addref`; the clone
/// releases when it drops, so the node keeps exactly one owner.
fn release_handle(session: &NativeSession, key: RawSlot) -> bool {
    let cloned = {
        let refs = session.shared.refs.borrow();
        refs.as_ref()
            .and_then(|table| table.stored_clone(key))
            .expect("retained node")
    };
    session
        .shared
        .release_stored(&cloned)
        .expect("host release")
}
fn repeat_handle(session: &NativeSession, key: RawSlot) {
    let cloned = {
        let refs = session.shared.refs.borrow();
        refs.as_ref()
            .and_then(|table| table.stored_clone(key))
            .expect("retained node")
    };
    session
        .shared
        .retain_stored(cloned)
        .expect("repeat host retention");
}
#[test]
#[ignore = "Required native RefTable retention; actual original capture directory must be provided"]
#[expect(
    clippy::indexing_slicing,
    reason = "The recipe's table labels sit at fixed indexes of the held order"
)]
#[expect(
    clippy::too_many_lines,
    reason = "The whole original recipe is one retention narrative; splitting would fragment it"
)]
fn native_reftable_retention_matches_the_original_refs_receipt() {
    let raw = receipts::receipt("refs");
    let session = NativeSession::open(256).expect("materialized session");
    let realm = session.realm().clone();
    realm.heap.install_retired_trace(128);
    let mut tables = Vec::new();
    for label in ['A', 'B', 'C', 'D'] {
        let (id, key) = retain_table(&session);
        tables.push((label, id, key));
    }
    // `roots object A..D`: the RefTable node is the sole owner of each
    // table, with an empty extent.
    let snapshot = session.shared.snapshot();
    assert_eq!(snapshot.heap.chain.len(), 89);
    assert_eq!(snapshot.ref_slots, 4);
    assert_eq!(snapshot.ref_used, 4);
    for (label, id, _) in &tables {
        let counts = observation::counts(&snapshot.heap, identity(*id)).expect("counts");
        assert_eq!(counts.references, 1, "table {label}");
        assert_eq!(counts.external, 0, "table {label}");
        assert_eq!(realm.heap.table_used(*id), Some(0), "table {label}");
    }
    // The allocation-order chain matches occurrence zero exactly.
    let wanted = observation::receipt_labels(&raw, 0);
    let labels = tables_labels(&tables);
    assert_eq!(
        observation::labeled_positions(&snapshot, &labels, &wanted),
        receipts::chain_positions(&raw, 0),
        "the retained tables prepend in allocation order before the base graph"
    );
    // `roots object repeat`: a repeated sq_addref leaves exactly one owner
    // and grows only the node handle count.
    repeat_handle(&session, tables[0].2);
    let snapshot = session.shared.snapshot();
    assert_eq!(snapshot.ref_used, 4, "no new node for a repeated handle");
    assert_eq!(
        observation::counts(&snapshot.heap, identity(tables[0].1)).expect("A"),
        crate::heap::observation::Counts {
            references: 1,
            external: 0
        }
    );
    // `roots release intermediate 0` and `roots object remaining`.
    assert!(!release_handle(&session, tables[0].2));
    assert_eq!(
        observation::counts(&session.shared.snapshot().heap, identity(tables[0].1)).expect("A"),
        crate::heap::observation::Counts {
            references: 1,
            external: 0
        }
    );
    // `roots collected retained-tokens 0`: retained tokens are marked, and
    // the rebuilt chain places them right after the primary subtree in
    // ascending physical slot order.
    assert_eq!(
        session.collector().collect(),
        Ok(receipts::collected(&raw, "retained-tokens"))
    );
    let snapshot = session.shared.snapshot();
    let wanted = observation::receipt_labels(&raw, 1);
    assert_eq!(
        observation::labeled_positions(&snapshot, &labels, &wanted),
        receipts::chain_positions(&raw, 1),
        "the rebuilt chain enqueues the reftable objects after V in slot order"
    );
    // `roots release last 1`: the final release of B frees the table.
    assert!(release_handle(&session, tables[1].2));
    assert!(realm.heap.payload_address(tables[1].1).is_none());
    assert_eq!(session.shared.snapshot().ref_used, 3);
    // Five more tables: E reuses the freed hole, F doubles the full table
    // and the resize re-adds the old nodes ascending with their refs.
    let mut grown = tables.clone();
    for label in ['E', 'F', 'G', 'H', 'I'] {
        let (id, key) = retain_table(&session);
        grown.push((label, id, key));
    }
    assert_eq!(session.shared.snapshot().ref_slots, 8);
    assert_eq!(session.shared.snapshot().ref_used, 8);
    assert_eq!(
        session.collector().collect(),
        Ok(receipts::collected(&raw, "reused-grown"))
    );
    let snapshot = session.shared.snapshot();
    assert_eq!(snapshot.heap.chain.len(), 93);
    let labels = tables_labels(&grown);
    let wanted = observation::receipt_labels(&raw, 2);
    assert_eq!(
        observation::labeled_positions(&snapshot, &labels, &wanted),
        receipts::chain_positions(&raw, 2),
        "hole reuse and the doubling resize preserve the ascending slot order"
    );
    // Releasing every remaining handle frees A, C, D, E, F, G, H, I in the
    // held order of the original recipe.
    for (label, id, key) in grown.iter().filter(|(label, _, _)| *label != 'B') {
        assert!(release_handle(&session, *key), "final release of {label}");
        assert!(realm.heap.payload_address(*id).is_none(), "freed {label}");
    }
    assert_eq!(session.shared.snapshot().ref_used, 0);
    assert_eq!(
        session.collector().collect(),
        Ok(receipts::collected(&raw, "tokens-released"))
    );
    assert_eq!(session.shared.snapshot().heap.chain.len(), 85);
    session.close();
    // The teardown frees the retained tables in the held order, then the
    // base graph: B first, then A, C, D, E, F, G, H, I, then O and V.
    let retired = realm.heap.retired_identities();
    let order: Vec<char> = retired
        .iter()
        .filter_map(|id| {
            grown
                .iter()
                .find(|(_, candidate, _)| {
                    candidate.index == id.index && candidate.generation == id.generation
                })
                .map(|(label, _, _)| *label)
        })
        .collect();
    assert_eq!(order, ['B', 'A', 'C', 'D', 'E', 'F', 'G', 'H', 'I']);
}
fn tables_labels(tables: &[(char, ObjectId, RawSlot)]) -> Vec<(char, ObjectId)> {
    tables.iter().map(|(label, id, _)| (*label, *id)).collect()
}
#[test]
#[ignore = "Required native RefTable string retention; actual original capture directory must be provided"]
fn native_reftable_string_slots_occupy_storage_but_never_mark() {
    let raw = receipts::receipt("refs-string");
    let session = NativeSession::open(256).expect("materialized session");
    let realm = session.realm().clone();
    realm.heap.install_retired_trace(128);
    // The observer's recipe retains A, the interned string S, then B, C; D
    // grows the full four-slot table (`native_roots_refs.hpp`).
    let (a_id, a_key) = retain_table(&session);
    let string = Stored::Scalar(Scalar::String(realm.string(b"ref-slot")));
    session
        .shared
        .retain_stored(string.clone())
        .expect("string retention");
    let string_key = RawSlot::of_stored(&string, session.realm())
        .expect("refcounted string")
        .expect("identity");
    drop(string);
    let mut tables = vec![('A', a_id, a_key)];
    for label in ['B', 'C', 'D'] {
        let (id, key) = retain_table(&session);
        tables.push((label, id, key));
    }
    // Five occupied slots: the string shares the physical storage without
    // joining the collectable chain.
    let snapshot = session.shared.snapshot();
    assert_eq!(snapshot.heap.chain.len(), 89);
    assert_eq!(snapshot.ref_slots, 8, "D doubled the four-slot table");
    assert_eq!(snapshot.ref_used, 5);
    // `roots collected retained-tokens 0`: the rebuilt chain marks the
    // four tables and skips the string slot exactly like the receipt.
    assert_eq!(
        session.collector().collect(),
        Ok(receipts::collected(&raw, "retained-tokens"))
    );
    let snapshot = session.shared.snapshot();
    let wanted = observation::receipt_labels(&raw, 1);
    assert_eq!(
        observation::labeled_positions(&snapshot, &tables_labels(&tables), &wanted),
        receipts::chain_positions(&raw, 1),
        "the string occupies its slot but never enters the rebuilt chain"
    );
    // The string release is the last free of the recipe, after every table.
    for (label, id, key) in &tables {
        assert!(release_handle(&session, *key), "final release of {label}");
        assert!(realm.heap.payload_address(*id).is_none(), "freed {label}");
    }
    assert!(release_handle(&session, string_key));
    assert_eq!(session.shared.snapshot().ref_used, 0);
    assert_eq!(
        session.collector().collect(),
        Ok(receipts::collected(&raw, "tokens-released"))
    );
    assert_eq!(session.shared.snapshot().heap.chain.len(), 85);
    session.close();
}
