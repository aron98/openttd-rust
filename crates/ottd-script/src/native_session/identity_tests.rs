//! Stable actual-payload identity witnesses (design §121-123): address
//! format, alias stability, arena-growth stability, lifetime at retirement
//! and read/hash effects, all against live heap storage.
use super::ref_identity::{OT_STRING, OT_TABLE, OT_THREAD, RawSlot};
use crate::{Realm, heap::Object, heap::Stored, table, table::scalar::Scalar};

fn fresh_table(realm: &Realm) -> Stored {
    let unpublished = realm
        .heap
        .unpublished(realm, Object::Table(table::Data::new(0).expect("table")))
        .expect("table allocation");
    Stored::Table(unpublished.publish_shared())
}
fn key_of(realm: &Realm, stored: &Stored) -> RawSlot {
    RawSlot::of_stored(stored, realm)
        .expect("refcounted object")
        .expect("identity")
}
fn identity_of(stored: &Stored) -> crate::heap::ObjectId {
    stored.edge().expect("edge").identity()
}
#[test]
fn live_payload_addresses_are_aligned_and_unique_per_object() {
    let realm = Realm::new();
    let first = fresh_table(&realm);
    let second = fresh_table(&realm);
    let first_key = key_of(&realm, &first);
    let second_key = key_of(&realm, &second);
    // Address format: a real allocation of the object's own storage, so it
    // is non-zero and aligned to the payload allocation.
    assert_ne!(first_key.raw(), 0);
    assert_eq!(
        first_key.raw() % u64::try_from(std::mem::align_of::<Object>()).expect("align"),
        0
    );
    // Distinct live objects differ; every alias of one object reads the same.
    assert_ne!(first_key.raw(), second_key.raw());
    assert_eq!(key_of(&realm, &first).raw(), first_key.raw());
    assert_eq!(
        realm.heap.payload_address(identity_of(&first)),
        Some(first_key.raw())
    );
    drop(first);
    drop(second);
}
#[test]
fn payload_addresses_survive_arena_growth_and_stay_readable() {
    let realm = Realm::new();
    let table = fresh_table(&realm);
    let key = key_of(&realm, &table);
    // Growing the entry arena moves arena slots, never the stable payload
    // allocations the addresses read.
    for _ in 0..64 {
        fresh_table(&realm);
    }
    assert_eq!(
        realm.heap.payload_address(identity_of(&table)),
        Some(key.raw())
    );
    assert_eq!(key_of(&realm, &table).raw(), key.raw());
    drop(table);
}
#[test]
fn payload_addresses_end_at_actual_retirement() {
    let realm = Realm::new();
    let table = fresh_table(&realm);
    let identity = identity_of(&table);
    let before = realm.heap.snapshot();
    drop(table);
    realm.heap.drain_releases();
    // The token lifetime ends at actual retirement: the identity reads as
    // absent even though the arena slot still exists.
    assert_eq!(realm.heap.payload_address(identity), None);
    let after = realm.heap.snapshot();
    assert_ne!(before, after);
    // Reuse of the retired address by a later allocation is permitted, so
    // only the presence of a fresh live address, not a fixed next value, is
    // asserted.
    let replacement = fresh_table(&realm);
    assert!(
        realm
            .heap
            .payload_address(identity_of(&replacement))
            .is_some()
    );
}
#[test]
fn pointer_keys_hash_by_the_address_shift_three() {
    let realm = Realm::new();
    let table = fresh_table(&realm);
    let key = key_of(&realm, &table);
    // `sqtable.h:13 hashptr` and the exact table tag of `squirrel.h:119`.
    assert_eq!(key.hash(), key.raw() >> 3);
    assert_eq!(key.tag(), OT_TABLE);
    drop(table);
}
#[test]
fn interned_strings_hash_by_their_native_string_hash() {
    let realm = Realm::new();
    let first = Scalar::String(realm.string(b"ref-slot"));
    let alias = Scalar::String(realm.string(b"ref-slot"));
    let other = Scalar::String(realm.string(b"other-slot"));
    let first_key = RawSlot::of_string(&first).expect("string key");
    let alias_key = RawSlot::of_string(&alias).expect("string key");
    let other_key = RawSlot::of_string(&other).expect("string key");
    // One interned string: same live allocation, so the same raw identity
    // for every alias, and a distinct one for a different string.
    assert_eq!(first_key.raw(), alias_key.raw());
    assert_ne!(first_key.raw(), other_key.raw());
    // The hash is the interned native `_hash` of the bytes (`sqtable.h:18`).
    assert_eq!(first_key.hash(), first.hash().expect("native string hash"));
    assert_eq!(other_key.hash(), other.hash().expect("native string hash"));
    assert_eq!(first_key.tag(), OT_STRING);
}
#[test]
fn retained_contexts_carry_thread_type_tags() {
    let realm = Realm::new();
    let context = realm
        .heap
        .unpublished(
            &realm,
            Object::Context(Box::new(super::payload::ContextPayload::allocated())),
        )
        .expect("context allocation");
    let stored = Stored::Context(context.publish_shared());
    let key = RawSlot::of_stored(&stored, &realm)
        .expect("refcounted")
        .expect("identity");
    // `squirrel.h:126`: OT_THREAD = _RT_THREAD | SQOBJECT_REF_COUNTED.
    assert_eq!(key.tag(), OT_THREAD);
    assert_ne!(key.raw(), 0);
    drop(stored);
}
