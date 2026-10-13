//! Idle-session construction, capability refusal and close-order witnesses.
use super::context::{ContextPhase, RootMembership};
use super::observation::{self, SessionSnapshot};
use crate::NativeSession;
use crate::heap::observation::ObjectKind;
use crate::{NativeCapability, NativeContextError, RootEnvironment, Value};

fn snapshot(session: &NativeSession) -> SessionSnapshot {
    session.shared.snapshot()
}
#[test]
fn open_rejects_invalid_initial_slots_before_any_allocation() {
    assert!(matches!(
        NativeSession::open(0),
        Err(NativeContextError::InvalidInitialSlots)
    ));
}
#[test]
fn idle_graph_accounts_for_exactly_eighty_five_collectables() {
    let session = NativeSession::open(256).expect("materialized session");
    let snapshot = snapshot(&session);
    assert_eq!(snapshot.heap.chain.len(), 85);
    assert_eq!(
        observation::kind_count(&snapshot.heap, ObjectKind::Table),
        14,
        "thirteen shared tables plus the root table"
    );
    assert_eq!(
        observation::kind_count(&snapshot.heap, ObjectKind::NativeClosure),
        70,
        "sixty-six default closures plus four base closures"
    );
    assert_eq!(
        observation::kind_count(&snapshot.heap, ObjectKind::Context),
        1
    );
    assert!(snapshot.heap.entries.iter().all(|entry| {
        entry
            .life
            .is_none_or(|counts| counts.references == 1 && counts.external == 0)
    }));
    session.close();
}
#[test]
fn idle_primary_and_shared_fields_hold_single_owners() {
    let session = NativeSession::open(256).expect("materialized session");
    let snapshot = snapshot(&session);
    // The primary context keeps exactly its `_root_vm` owner.
    let vm = snapshot.vm.expect("idle scalars");
    assert_eq!(vm.references, 1);
    assert_eq!(vm.stack_size, 256);
    assert_eq!(vm.top, 0);
    assert_eq!(vm.stackbase, 0);
    assert_eq!(vm.vargs_size, 0);
    assert_eq!(vm.callsstacksize, 0);
    assert_eq!(vm.etraps, 0);
    assert_eq!(vm.ops, 0);
    assert_eq!(snapshot.context.1, ContextPhase::Live);
    assert_eq!(snapshot.context.2, 1);
    // The registry's scalar membership is authoritative and matches the
    // observed persistent fields; the shared `_root_vm` field targets the
    // primary context itself.
    assert_eq!(
        snapshot.memberships.1,
        RootMembership::Context(snapshot.context.0),
        "the _root_vm membership targets the primary context"
    );
    assert!(
        snapshot
            .memberships
            .0
            .iter()
            .zip(snapshot.fields.iter().map(|(_, membership)| *membership))
            .all(|(registry, field)| registry == &field),
        "registry memberships match the persistent fields"
    );
    // Four allocated call records carry their source defaults; none is an
    // active marker root, matching the receipt's zero call-stack size.
    assert_eq!(snapshot.call_records.len(), 4);
    for record in &snapshot.call_records {
        assert!(!record.root);
        assert_eq!(record.etraps, 0);
        assert_eq!(record.prevbase, 0);
        assert_eq!(record.prevtop, 0);
        assert_eq!(record.target, 0);
        assert_eq!(record.ncalls, 0);
        assert_eq!(record.varg_base, 0);
        assert_eq!(record.varg_size, 0);
    }
    // Persistent memberships: only the root field holds a collectable.
    let memberships = snapshot
        .fields
        .iter()
        .map(|(_, membership)| *membership)
        .collect::<Vec<_>>();
    let Some([error, handler, debug, root, temp]) = memberships.first_chunk() else {
        panic!("five persistent fields")
    };
    assert_eq!(*error, RootMembership::Null);
    assert_eq!(*handler, RootMembership::Null);
    assert_eq!(*debug, RootMembership::Null);
    assert!(matches!(root, RootMembership::Object(_)));
    assert_eq!(*temp, RootMembership::Null);
    // Every labeled table holds exactly one owner.
    for (label, identity) in &snapshot.labels {
        let counts = observation::counts(&snapshot.heap, *identity).expect("counts");
        assert_eq!(counts.references, 1, "label {label}");
    }
    session.close();
}
#[test]
fn idle_shared_tables_carry_their_ledger_extents() {
    let session = NativeSession::open(256).expect("materialized session");
    let snapshot = snapshot(&session);
    for (label, expected) in [
        ('R', 0),
        ('C', 0),
        ('M', 18),
        ('T', 8),
        ('A', 15),
        ('S', 9),
        ('N', 5),
        ('F', 8),
        ('G', 3),
        ('H', 5),
        ('K', 6),
        ('I', 4),
        ('W', 3),
        ('O', 8),
    ] {
        let identity = observation::label_identity(&snapshot, label).expect("label");
        assert_eq!(
            session.realm().heap.table_extent(identity),
            Some(expected),
            "table {label}"
        );
    }
    session.close();
}
#[test]
fn idle_ref_table_starts_with_four_empty_ascending_slots() {
    let session = NativeSession::open(256).expect("materialized session");
    let snapshot = snapshot(&session);
    assert_eq!(snapshot.ref_slots, 4);
    assert_eq!(snapshot.ref_used, 0);
    assert_eq!(
        snapshot.ref_free,
        vec![Some(0), Some(1), Some(2), Some(3), None],
        "the free chain ascends through all four slots"
    );
    session.close();
}
#[test]
fn capability_requests_are_refused_without_touching_state() {
    let session = NativeSession::open(256).expect("materialized session");
    let before = session.shared.snapshot().heap;
    for capability in [
        NativeCapability::Compiler,
        NativeCapability::Execution,
        NativeCapability::ActivePrint,
        NativeCapability::NonEmptyRefTable,
    ] {
        assert_eq!(
            session.request_capability(capability),
            Err(NativeContextError::IncompleteNativeGraph)
        );
    }
    let after = session.shared.snapshot().heap;
    assert_eq!(before, after);
    assert_eq!(session.collector().collect(), Ok(0));
    session.close();
}
#[test]
fn idle_collection_keeps_every_shared_owner_in_place() {
    let session = NativeSession::open(256).expect("materialized session");
    assert_eq!(session.collector().collect(), Ok(0));
    assert_eq!(snapshot(&session).heap.chain.len(), 85);
    // Ordinary realm collection dispatches to the native root enumerator.
    session.realm().collect_cycles();
    assert_eq!(snapshot(&session).heap.chain.len(), 85);
    session.close();
}
#[test]
fn idle_sessions_use_independent_realms_and_heaps() {
    let first = NativeSession::open(256).expect("first session");
    let second = NativeSession::open(1024).expect("second session");
    assert!(!first.realm().same(second.realm()));
    assert_eq!(
        second.shared.snapshot().vm.expect("scalars").stack_size,
        1024
    );
    assert_eq!(first.collector().collect(), Ok(0));
    assert_eq!(second.shared.snapshot().heap.chain.len(), 85);
    second.close();
    first.close();
}
#[test]
fn close_releases_owners_in_source_free_order() {
    let session = NativeSession::open(256).expect("materialized session");
    let realm = session.realm().clone();
    let snapshot = snapshot(&session);
    realm.heap.install_retired_trace(96);
    session.close();
    let retired = realm.heap.retired_identities();
    let labels = retired
        .iter()
        .filter_map(|identity| {
            snapshot
                .labels
                .iter()
                .find(|(_, candidate)| candidate == identity)
                .map(|(label, _)| *label)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        [
            'O', 'R', 'C', 'M', 'V', 'T', 'A', 'S', 'N', 'F', 'G', 'H', 'K', 'I', 'W'
        ],
        "sq_close frees the root first, then the shared destructor order"
    );
    let after = realm.heap.snapshot();
    assert!(after.chain.is_empty(), "every collectable is retired");
    assert!(after.entries.iter().all(|entry| entry.life.is_none()));
}
#[test]
fn closed_sessions_refuse_weak_collector_handles() {
    let session = NativeSession::open(256).expect("materialized session");
    let collector = session.collector();
    session.close();
    assert_eq!(collector.collect(), Err(NativeContextError::ClosedContext));
}
#[test]
fn materialized_root_registers_the_source_base_closures_and_widths() {
    let session = NativeSession::open(256).expect("materialized session");
    let root = observation::observation_root_table(&session).expect("root table");
    let owned = RootEnvironment::from_table(root);
    assert_eq!(
        owned.raw_get(b"_version_"),
        Some(Value::String(session.realm().string(
            b"Squirrel 2.2.5 stable - With custom OpenTTD modifications"
        ),))
    );
    assert_eq!(owned.raw_get(b"_charsize_"), Some(Value::Integer(1)));
    assert_eq!(owned.raw_get(b"_intsize_"), Some(Value::Integer(8)));
    assert_eq!(owned.raw_get(b"_floatsize_"), Some(Value::Integer(4)));
    for name in [b"assert".as_slice(), b"print", b"array", b"type"] {
        assert!(
            matches!(owned.raw_get(name), Some(Value::NativeClosure(_))),
            "base closure {name:?}"
        );
    }
    drop(owned);
    session.close();
}
