//! Physical `RefTable` algorithm witnesses from `sqstate.cpp`.
use super::ref_table::{RawSlot, RefTable};

fn token(raw: u64) -> RawSlot {
    RawSlot::new(raw, 1)
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
    let mut table = RefTable::new().expect("physical table");
    table.add_ref(token(8)).expect("add");
    table.add_ref(token(16)).expect("add");
    table.add_ref(token(24)).expect("add");
    assert_eq!(table.slotused(), 3);
    // Slots were taken in ascending free-chain order: 0, 1, 2.
    assert_eq!(
        table.free_chain(),
        vec![Some(3), None],
        "only the last physical slot stays free"
    );
}
#[test]
fn repeated_addref_only_increments_the_node_refcount() {
    let mut table = RefTable::new().expect("physical table");
    table.add_ref(token(8)).expect("add");
    table.add_ref(token(8)).expect("repeat");
    table.add_ref(token(8)).expect("repeat");
    assert_eq!(table.slotused(), 1, "no new node for a repeated handle");
    // Intermediate releases keep the node until its last handle is gone.
    assert_eq!(table.release(token(8)), Ok(false));
    assert_eq!(table.release(token(8)), Ok(false));
    assert_eq!(table.slotused(), 1);
    // The final release frees it and prepends the slot to the free chain.
    assert_eq!(table.release(token(8)), Ok(true));
    assert_eq!(table.slotused(), 0);
    assert_eq!(
        table.free_chain(),
        vec![Some(0), Some(1), Some(2), Some(3), None]
    );
}
#[test]
fn final_release_reuses_the_most_recently_freed_slot_first() {
    let mut table = RefTable::new().expect("physical table");
    table.add_ref(token(8)).expect("add");
    table.add_ref(token(16)).expect("add");
    assert_eq!(table.release(token(8)), Ok(true));
    assert_eq!(table.release(token(16)), Ok(true));
    // Freed slots are prepended, so slot 1 is taken before slot 0.
    table.add_ref(token(24)).expect("add");
    assert_eq!(table.slotused(), 1);
    assert_eq!(
        table.free_chain(),
        vec![Some(0), Some(2), Some(3), None],
        "hole reuse takes the freelist head first"
    );
}
#[test]
fn full_table_doubles_and_readds_old_nodes_ascending_with_their_refs() {
    let mut table = RefTable::new().expect("physical table");
    for raw in [8u64, 16, 24, 32] {
        table.add_ref(token(raw)).expect("add");
    }
    assert_eq!(table.slotused(), 4);
    assert_eq!(table.numofslots(), 4);
    table.add_ref(token(40)).expect("grow");
    assert_eq!(table.numofslots(), 8, "a full table doubles its storage");
    assert_eq!(table.slotused(), 5);
    // Repeated handles still share one node after the resize.
    table.add_ref(token(24)).expect("repeat");
    assert_eq!(table.slotused(), 5);
    // Both references must be released before the node leaves the table.
    assert_eq!(table.release(token(24)), Ok(false));
    assert_eq!(table.release(token(24)), Ok(true));
    assert_eq!(table.slotused(), 4);
}
#[test]
fn distinct_raw_values_with_equal_type_tags_do_not_alias() {
    let mut table = RefTable::new().expect("physical table");
    table.add_ref(token(8)).expect("add");
    table.add_ref(token(8 + 64)).expect("add");
    assert_eq!(table.slotused(), 2);
    // Type tags are part of the source equality, not just the raw identity.
    table.add_ref(RawSlot::new(8, 2)).expect("add");
    assert_eq!(table.slotused(), 3);
    assert_eq!(table.release(RawSlot::new(8, 2)), Ok(true));
    assert_eq!(table.slotused(), 2);
}
#[test]
fn release_of_an_absent_object_is_reported_false() {
    let mut table = RefTable::new().expect("physical table");
    assert_eq!(table.release(token(8)), Ok(false));
}
#[test]
fn finalize_nulls_every_stored_object_ascending() {
    let mut table = RefTable::new().expect("physical table");
    table.add_ref(token(8)).expect("add");
    table.add_ref(token(16)).expect("add");
    table.finalize();
    // The source's Finalize only nulls stored objects; it leaves the free
    // chain and the slot counter untouched.
    assert_eq!(table.slotused(), 2);
    assert_eq!(table.mark_objects().count(), 0);
    assert_eq!(
        table.free_chain(),
        vec![Some(2), Some(3), None],
        "the physical free chain survives finalize"
    );
}
