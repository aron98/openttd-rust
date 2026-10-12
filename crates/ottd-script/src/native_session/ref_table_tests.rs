//! Physical `RefTable` algorithm witnesses from `sqstate.cpp:367-515`.
use super::ref_identity::{OT_TABLE, OT_THREAD, RawSlot};
use super::ref_table::RefTable;
use crate::{Realm, heap::Object, heap::Stored, table};

/// One fresh empty table with its owning stored reference and raw key.
fn table_stored(realm: &Realm) -> (Stored, RawSlot) {
    let unpublished = realm
        .heap
        .unpublished(realm, Object::Table(table::Data::new(0).expect("table")))
        .expect("table allocation");
    let edge = unpublished.publish_shared();
    let stored = Stored::Table(edge);
    let key = RawSlot::of_stored(&stored, realm)
        .expect("refcounted table")
        .expect("identity");
    (stored, key)
}
/// Test-only direct key injection: real payload addresses cannot force
/// deterministic hash-bucket collisions, so the mechanics probes build keys
/// directly (`sqstate.cpp:467-487 Get`).
fn add_key(table: &mut RefTable, raw: u64, tag: u32) {
    table
        .add_ref(Stored::null(), RawSlot::new(raw, tag))
        .expect("add");
}
#[test]
fn four_initial_slots_build_an_ascending_free_chain() {
    let table = RefTable::new().expect("physical table");
    assert_eq!(table.numofslots(), 4);
    assert_eq!(table.slotused(), 0);
    assert_eq!(
        table.free_chain(),
        vec![Some(0), Some(1), Some(2), Some(3), None]
    );
}
#[test]
fn add_takes_the_first_free_slot_and_prepends_the_bucket() {
    let realm = Realm::new();
    let mut table = RefTable::new().expect("physical table");
    for _ in 0..3 {
        let (stored, key) = table_stored(&realm);
        table.add_ref(stored, key).expect("add");
    }
    assert_eq!(table.slotused(), 3);
    // Slots were taken in ascending free-chain order: 0, 1, 2, regardless
    // of the payload-address hashes (sqstate.cpp:454-465 Add).
    assert_eq!(
        table.free_chain(),
        vec![Some(3), None],
        "only the last physical slot stays free"
    );
}
#[test]
fn repeated_addref_only_increments_the_node_refcount() {
    let realm = Realm::new();
    let mut table = RefTable::new().expect("physical table");
    let (stored, key) = table_stored(&realm);
    table.add_ref(stored.clone(), key).expect("add");
    table.add_ref(stored.clone(), key).expect("repeat");
    table.add_ref(stored, key).expect("repeat");
    assert_eq!(table.slotused(), 1, "no new node for a repeated handle");
    assert_eq!(table.slot_refs(key), Some(3));
    // Intermediate releases keep the node until its last handle is gone.
    assert_eq!(table.release(key), Ok(false));
    assert_eq!(table.release(key), Ok(false));
    assert_eq!(table.slotused(), 1);
    // The final release frees it and prepends the slot to the free chain.
    assert_eq!(table.release(key), Ok(true));
    assert_eq!(table.slotused(), 0);
    assert_eq!(
        table.free_chain(),
        vec![Some(0), Some(1), Some(2), Some(3), None]
    );
}
#[test]
fn final_release_reuses_the_most_recently_freed_slot_first() {
    let realm = Realm::new();
    let mut table = RefTable::new().expect("physical table");
    let (first, first_key) = table_stored(&realm);
    let (second, second_key) = table_stored(&realm);
    table.add_ref(first, first_key).expect("add");
    table.add_ref(second, second_key).expect("add");
    assert_eq!(table.release(first_key), Ok(true));
    assert_eq!(table.release(second_key), Ok(true));
    // Freed slots are prepended, so slot 1 is taken before slot 0.
    let (third, third_key) = table_stored(&realm);
    table.add_ref(third, third_key).expect("add");
    assert_eq!(table.slotused(), 1);
    assert_eq!(
        table.free_chain(),
        vec![Some(0), Some(2), Some(3), None],
        "hole reuse takes the freelist head first"
    );
}
#[test]
#[expect(
    clippy::indexing_slicing,
    reason = "The recipe retains four fixed keys in order"
)]
fn full_table_doubles_and_readds_old_nodes_ascending_with_their_refs() {
    let realm = Realm::new();
    let mut table = RefTable::new().expect("physical table");
    let mut keys = Vec::new();
    for _ in 0..4 {
        let (stored, key) = table_stored(&realm);
        table.add_ref(stored, key).expect("add");
        keys.push(key);
    }
    assert_eq!(table.slotused(), 4);
    assert_eq!(table.numofslots(), 4);
    let (fifth, fifth_key) = table_stored(&realm);
    table.add_ref(fifth, fifth_key).expect("grow");
    assert_eq!(table.numofslots(), 8, "a full table doubles its storage");
    assert_eq!(table.slotused(), 5);
    // Repeated handles still share one node after the resize.
    assert_eq!(table.add_ref(Stored::null(), keys[2]), Ok(()));
    assert_eq!(table.slotused(), 5);
    // Both references must be released before the node leaves the table.
    assert_eq!(table.release(keys[2]), Ok(false));
    assert_eq!(table.release(keys[2]), Ok(true));
    assert_eq!(table.slotused(), 4);
}
#[test]
fn distinct_live_objects_with_equal_type_tags_do_not_alias() {
    let realm = Realm::new();
    let mut table = RefTable::new().expect("physical table");
    let (first, first_key) = table_stored(&realm);
    let (second, second_key) = table_stored(&realm);
    table.add_ref(first, first_key).expect("add");
    table.add_ref(second, second_key).expect("add");
    assert_eq!(table.slotused(), 2, "two live tables never share a node");
    // The exact type tag is part of the source equality, so the same raw
    // value under a different type is a distinct node (sqstate.cpp:473-474).
    let (other, other_key) = table_stored(&realm);
    drop(other);
    add_key(&mut table, other_key.raw(), OT_THREAD);
    assert_eq!(table.slotused(), 3);
    assert_eq!(
        table.release(RawSlot::new(other_key.raw(), OT_THREAD)),
        Ok(true)
    );
    assert_eq!(table.slotused(), 2);
}
#[test]
fn release_of_an_absent_object_is_reported_false() {
    let realm = Realm::new();
    let mut table = RefTable::new().expect("physical table");
    let (absent, absent_key) = table_stored(&realm);
    drop(absent);
    assert_eq!(table.release(absent_key), Ok(false));
}
#[test]
fn middle_bucket_nodes_relink_their_predecessor_on_release() {
    let mut table = RefTable::new().expect("physical table");
    // Raw 8 and 40 both hash to bucket 1 of the four-slot table
    // (`hashptr` shifts three: 1 and 5 masked to 1).
    add_key(&mut table, 8, OT_TABLE);
    add_key(&mut table, 40, OT_TABLE);
    assert_eq!(table.slotused(), 2);
    assert_eq!(table.release(RawSlot::new(8, OT_TABLE)), Ok(true));
    assert_eq!(table.slotused(), 1);
    // The remaining node is still found through its bucket head.
    assert_eq!(table.release(RawSlot::new(40, OT_TABLE)), Ok(true));
    assert_eq!(table.slotused(), 0);
}
#[test]
fn finalize_nulls_every_stored_object_ascending() {
    let realm = Realm::new();
    let mut table = RefTable::new().expect("physical table");
    let (first, first_key) = table_stored(&realm);
    let (second, second_key) = table_stored(&realm);
    table.add_ref(first, first_key).expect("add");
    table.add_ref(second, second_key).expect("add");
    let owners = table.finalize();
    // The source's Finalize only nulls stored objects; it leaves the free
    // chain and the slot counter untouched (sqstate.cpp:372-378).
    assert_eq!(table.slotused(), 2);
    assert_eq!(table.mark_targets().count(), 0);
    assert_eq!(
        table.free_chain(),
        vec![Some(2), Some(3), None],
        "the physical free chain survives finalize"
    );
    // Releasing the taken owners retires both tables from the heap.
    let mut retired = Vec::new();
    for owner in owners {
        let identity = owner.edge().expect("table edge").identity();
        drop(owner);
        realm.heap.drain_releases();
        retired.push(
            realm
                .heap
                .snapshot()
                .entries
                .iter()
                .find(|entry| {
                    entry.identity.index == identity.index
                        && entry.identity.generation == identity.generation
                })
                .expect("retired entry")
                .life,
        );
    }
    assert!(retired.iter().all(std::option::Option::is_none));
}
