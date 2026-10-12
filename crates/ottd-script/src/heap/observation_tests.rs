//! Snapshots cannot retain owners, drain retirement, or confuse reused generations.
use super::{Operand, observation::ObjectKind};
use crate::{Realm, Value};

#[test]
fn snapshots_preserve_ownership_release_and_generation_reuse() {
    let realm = Realm::new();
    let table = realm.table().expect("table");
    let id = table.0.observation_identity();
    let before = realm.heap.snapshot();
    assert_eq!(before.chain, [id]);
    let state = before.entries.first().expect("entry");
    assert_eq!(state.kind, Some(ObjectKind::Table));
    assert_eq!(state.life.expect("life").references, 1);
    assert_eq!(state.life.expect("life").external, 1);
    *realm.heap.retired.borrow_mut() = Some(Vec::with_capacity(16));
    assert_eq!(realm.heap.snapshot(), before);
    let copy = table.clone();
    let held = realm.heap.snapshot();
    assert_eq!(
        held.entries
            .first()
            .expect("entry")
            .life
            .expect("life")
            .references,
        2
    );
    assert_eq!(table.0.references(), 2);
    drop(copy);
    assert_eq!(realm.heap.snapshot(), before);
    assert!(
        realm
            .heap
            .retired
            .borrow()
            .as_ref()
            .expect("trace")
            .is_empty()
    );
    drop(table);
    let retired = realm.heap.snapshot();
    assert!(retired.chain.is_empty());
    assert_eq!(retired.entries.first().expect("entry").life, None);
    assert_eq!(
        realm.heap.retired.borrow().as_ref().expect("trace").len(),
        1
    );
    let replacement = realm.table().expect("replacement");
    let next = replacement.0.observation_identity();
    assert_eq!(next.index, id.index);
    assert_ne!(next.generation, id.generation);
    assert_eq!(
        before.entries.first().expect("old scalar copy").identity,
        id
    );
}

#[test]
fn reference_live_finalized_storage_is_observed_without_an_owner() {
    let realm = Realm::new();
    let table = realm.table().expect("table");
    let id = table.0.observation_identity();
    table
        .new_slot(Value::Integer(1), Value::Integer(7))
        .expect("slot");
    let retained = Operand::new(Value::Table(table)).expect("reference only");
    let before = realm.heap.snapshot();
    assert_eq!(before.chain, [id]);
    assert_eq!(
        before
            .entries
            .first()
            .expect("entry")
            .life
            .expect("life")
            .external,
        0
    );
    assert_eq!(realm.heap.collect(), 1);
    let finalized = realm.heap.snapshot();
    let state = finalized.entries.first().expect("entry");
    assert_eq!(state.kind, Some(ObjectKind::Table));
    assert!(state.reference_live_off_chain);
    assert_eq!(state.life.expect("life").references, 1);
    assert_eq!(realm.heap.snapshot(), finalized);
    drop(retained);
    assert_eq!(
        realm.heap.snapshot().entries.first().expect("entry").life,
        None
    );
}

#[test]
fn observation_off_and_on_have_the_same_collection_and_retirement_order() {
    fn run(observe: bool) -> (usize, Vec<super::ObjectId>) {
        let realm = Realm::new();
        let parent = realm.table().expect("parent");
        let child = realm.table().expect("child");
        parent.set_delegate(Some(&child)).expect("delegate");
        parent
            .new_slot(Value::Integer(0), Value::Table(parent.clone()))
            .expect("self");
        *realm.heap.retired.borrow_mut() = Some(Vec::with_capacity(16));
        if observe {
            let snapshot = realm.heap.snapshot();
            assert_eq!(
                snapshot.chain,
                [
                    child.0.observation_identity(),
                    parent.0.observation_identity()
                ]
            );
            assert_eq!(snapshot, realm.heap.snapshot());
        }
        drop(child);
        drop(parent);
        if observe {
            let _state = realm.heap.snapshot();
        }
        let collected = realm.heap.collect();
        if observe {
            let _state = realm.heap.snapshot();
        }
        let events = realm.heap.retired.borrow_mut().take().expect("events");
        (collected, events)
    }
    let off = run(false);
    assert_eq!(off.0, 2);
    assert_eq!(off.1.len(), 2);
    assert_eq!(off, run(true));
}
