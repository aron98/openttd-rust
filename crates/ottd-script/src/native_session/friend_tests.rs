//! Retained-friend allocation, retention, lease and validation witnesses.
use super::context::ContextPhase;
use super::observation::{self, SessionSnapshot};
use crate::heap::observation::ObjectKind;
use crate::{NativeCapability, NativeContextError, NativeFriendField, NativeSession};

fn snapshot(session: &NativeSession) -> SessionSnapshot {
    session.shared.snapshot()
}
#[test]
fn retain_friend_rejects_a_zero_sized_stack_before_any_allocation() {
    let mut session = NativeSession::open(256).expect("materialized session");
    let before = snapshot(&session).heap;
    assert!(matches!(
        session.retain_friend(0),
        Err(NativeContextError::InvalidInitialSlots)
    ));
    assert_eq!(before, snapshot(&session).heap);
    session.close();
}
#[test]
fn retain_friend_allocates_and_retains_one_real_friend_context() {
    let mut session = NativeSession::open(256).expect("materialized session");
    let token = session.retain_friend(256).expect("retained friend");
    let snapshot = snapshot(&session);
    // The friend context joins the heap as a real collectable, the only
    // owner being the RefTable node's reference.
    assert_eq!(
        observation::kind_count(&snapshot.heap, ObjectKind::Context),
        2,
        "the primary and the friend contexts"
    );
    assert_eq!(snapshot.heap.chain.len(), 86);
    assert_eq!(snapshot.friends.len(), 1);
    let (id, phase, references) = snapshot.friends.first().copied().expect("friend record");
    assert_eq!(id, token.context_id());
    assert_eq!(phase, ContextPhase::Live);
    assert_eq!(references, 1);
    assert_eq!(snapshot.ref_used, 1);
    assert_eq!(snapshot.ref_slots, 4);
    // The friend's Init copied the primary's roottable field, so the shared
    // root table now has exactly two owners (`sqvm.cpp:330`).
    let root = observation::label_identity(&snapshot, 'O').expect("root identity");
    let counts = observation::counts(&snapshot.heap, root).expect("root counts");
    assert_eq!(counts.references, 2);
    assert_eq!(counts.external, 0);
    // The base closures were re-registered by the friend's Init: the shared
    // root table keeps eight slots and exactly seventy live closures.
    let root_id = crate::heap::ObjectId {
        index: root.index,
        generation: root.generation,
    };
    assert_eq!(session.realm().heap.table_used(root_id), Some(8));
    assert_eq!(
        observation::kind_count(&snapshot.heap, ObjectKind::NativeClosure),
        70
    );
    drop(token);
    session.close();
}
#[test]
fn friend_view_observes_the_receipt_shaped_idle_scalars() {
    let mut session = NativeSession::open(256).expect("materialized session");
    let token = session.retain_friend(256).expect("retained friend");
    let view = session.friend(&token).expect("friend view");
    assert_eq!(view.context_id(), token.context_id());
    let scalars = view.scalars();
    assert_eq!(scalars.references, 1);
    assert!(!scalars.is_root);
    assert_eq!(scalars.stack_size, 256);
    assert_eq!(scalars.top, 0);
    assert_eq!(scalars.stackbase, 0);
    assert_eq!(scalars.vargs_size, 0);
    assert_eq!(scalars.callsstacksize, 0);
    assert_eq!(scalars.etraps, 0);
    assert_eq!(scalars.ops, 0);
    // error, handler, debug and temp stay Null; the root field co-owns the
    // shared root table with two references and extent eight.
    assert_eq!(
        scalars.persistent,
        [
            NativeFriendField::Null,
            NativeFriendField::Null,
            NativeFriendField::Null,
            NativeFriendField::Table {
                references: 2,
                extent: 8
            },
            NativeFriendField::Null,
        ]
    );
    drop(view);
    drop(token);
    session.close();
}
#[test]
fn the_view_lease_keeps_the_payload_address_and_survives_collection() {
    let mut session = NativeSession::open(256).expect("materialized session");
    let token = session.retain_friend(256).expect("retained friend");
    let friend_object = token.object;
    let leased_address = session
        .realm()
        .heap
        .payload_address(friend_object)
        .expect("live payload");
    let collector = session.collector();
    let view = session.friend(&token).expect("friend view");
    // Collection during the view cannot finalize the host-rooted payload:
    // the RefTable keeps the detached-for-borrow context marked.
    assert_eq!(collector.collect(), Ok(0));
    let scalars = view.scalars();
    assert_eq!(scalars.references, 1);
    drop(view);
    // The payload reattached allocation-free at the same stable address.
    assert_eq!(
        session.realm().heap.payload_address(friend_object),
        Some(leased_address)
    );
    drop(token);
    session.close();
}
#[test]
fn explicit_retain_repeats_one_addref_and_tokens_release_in_order() {
    let mut session = NativeSession::open(256).expect("materialized session");
    let first = session.retain_friend(256).expect("retained friend");
    let friend_object = first.object;
    let second = session.retain(&first).expect("repeated retention");
    let key = first.raw_slot();
    assert_eq!(
        session
            .shared
            .refs
            .borrow()
            .as_ref()
            .map(|table| table.slot_refs(key)),
        Some(Some(2)),
        "a repeated explicit retain only grows the handle count"
    );
    // The intermediate release keeps the friend and its record alive.
    drop(first);
    assert_eq!(session.shared.snapshot().friends.len(), 1);
    assert_eq!(
        session
            .shared
            .refs
            .borrow()
            .as_ref()
            .map(|table| table.slot_refs(key)),
        Some(Some(1))
    );
    assert!(
        session
            .realm()
            .heap
            .payload_address(friend_object)
            .is_some(),
        "the friend stays live through the intermediate release"
    );
    // The final release retires the friend context and frees its registry
    // slot; the identity reads absent after actual retirement.
    drop(second);
    let snapshot = snapshot(&session);
    assert!(snapshot.friends.is_empty());
    assert_eq!(snapshot.ref_used, 0);
    assert!(
        session
            .realm()
            .heap
            .payload_address(friend_object)
            .is_none()
    );
    session.close();
}
#[test]
fn friend_tokens_validate_realm_generation_and_lifecycle() {
    let mut first = NativeSession::open(256).expect("first session");
    let mut second = NativeSession::open(256).expect("second session");
    let foreign = second.retain_friend(256).expect("foreign token");
    assert!(matches!(
        first.friend(&foreign),
        Err(NativeContextError::ForeignContext)
    ));
    let token = first.retain_friend(256).expect("retained friend");
    let retired_id = token.context_id();
    // Reuse increments the identity generation of a retired registry slot
    // without wrapping (design §45).
    drop(token);
    let fresh = first.retain_friend(256).expect("reused slot friend");
    assert_eq!(fresh.context_id().index, retired_id.index);
    assert_eq!(fresh.context_id().generation, retired_id.generation + 1);
    // Closing the session invalidates every remaining token inertly: no
    // session borrow survives close, and weak capabilities fail closed.
    let collector = first.collector();
    first.close();
    assert_eq!(collector.collect(), Err(NativeContextError::ClosedContext));
    drop(fresh);
    drop(foreign);
    second.close();
}
#[test]
fn retained_friends_do_not_grant_execution_capabilities() {
    let mut session = NativeSession::open(256).expect("materialized session");
    let token = session.retain_friend(256).expect("retained friend");
    // Every capability request still refuses before any mutation.
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
    drop(token);
    session.close();
}
#[test]
fn close_releases_live_friend_tokens_inertly_and_retires_friends() {
    let mut session = NativeSession::open(256).expect("materialized session");
    let realm = session.realm().clone();
    let token = session.retain_friend(256).expect("retained friend");
    realm.heap.install_retired_trace(128);
    session.close();
    // The RefTable close released the friend's sole owner; the registry
    // record is retired and the later token drop stays memory-safe inert.
    let heap = realm.heap.snapshot();
    assert_eq!(
        heap.entries
            .iter()
            .filter(|entry| entry.kind == Some(ObjectKind::Context))
            .count(),
        0,
        "no context survives close"
    );
    assert!(heap.chain.is_empty());
    drop(token);
}
