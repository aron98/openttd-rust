//! Original delegate ownership, marking and destruction observations.
use super::owner_probe::{Probe, native};
use crate::{Realm, Value, VmError};
use std::fmt::Write;

#[test]
fn ordinary_gc_and_rooted_graph_orders_match_original() {
    for (mode, expected) in [
        (
            "ordinary",
            include_str!("../../tests/delegate-owners/ordinary.txt"),
        ),
        ("gc", include_str!("../../tests/delegate-owners/gc.txt")),
        (
            "marked",
            include_str!("../../tests/delegate-owners/marked.txt"),
        ),
    ] {
        let realm = Realm::new();
        let root = realm.table().expect("native root");
        let mut probe = Probe::new(&realm.heap);
        let a = realm.table().expect("A");
        probe.register('A', &a);
        let b = realm.table().expect("B");
        probe.register('B', &b);
        let d = realm.table().expect("D");
        probe.register('D', &d);
        let o = realm.table().expect("O");
        probe.register('O', &o);
        o.new_slot(Value::Integer(0), Value::Table(a))
            .expect("A edge");
        o.new_slot(Value::Integer(1), Value::Table(b))
            .expect("B edge");
        o.set_delegate(Some(&d)).expect("delegate");
        drop(d);
        if mode != "ordinary" {
            o.new_slot(Value::Integer(2), Value::Table(o.clone()))
                .expect("cycle");
        }
        probe.snapshot("attached").expect("snapshot");
        if mode == "marked" {
            root.new_slot(Value::Integer(77), Value::Table(o))
                .expect("root edge");
            writeln!(probe.output, "collected reachable {}", realm.heap.collect()).expect("count");
            probe.snapshot("rooted").expect("snapshot");
            probe.chain().expect("chain");
            drop(root.delete_slot(Value::Integer(77)).expect("remove root"));
        } else {
            drop(o);
        }
        if mode != "ordinary" {
            writeln!(
                probe.output,
                "collected unreachable {}",
                realm.heap.collect()
            )
            .expect("count");
        }
        probe.snapshot("released").expect("snapshot");
        probe.finish();
        assert_eq!(
            probe.output,
            native(mode, expected).expect("native receipt"),
            "{mode}"
        );
    }
}

#[test]
fn same_edge_replacement_clone_and_cycle_refusal_match_original() {
    let realm = Realm::new();
    let mut probe = Probe::new(&realm.heap);
    let d = realm.table().expect("D");
    probe.register('D', &d);
    let n = realm.table().expect("N");
    probe.register('N', &n);
    let o = realm.table().expect("O");
    probe.register('O', &o);
    let d_id = d.0.edge.life.id;
    o.set_delegate(Some(&d)).expect("attach");
    drop(d);
    probe.snapshot("attached").expect("snapshot");
    realm
        .heap
        .attach_delegate_id(&o.0, Some(d_id))
        .expect("same borrowed delegate with no host owner");
    probe.snapshot("same").expect("snapshot");
    o.set_delegate(Some(&n)).expect("replace");
    drop(n);
    probe.snapshot("replaced").expect("snapshot");
    let copy = o.shallow_clone().expect("clone");
    probe.register('C', &copy);
    probe.snapshot("cloned").expect("snapshot");
    assert_eq!(o.set_delegate(Some(&o)), Err(VmError::DelegateCycle));
    assert_eq!(
        o.delegate().expect("parent").set_delegate(Some(&o)),
        Err(VmError::DelegateCycle)
    );
    probe.output.push_str("cycle refusal 0 0\n");
    probe.snapshot("cycles").expect("snapshot");
    o.set_delegate(None).expect("detach");
    probe.snapshot("detached").expect("snapshot");
    drop(o);
    probe.snapshot("owner released").expect("snapshot");
    drop(copy);
    probe.snapshot("clone released").expect("snapshot");
    probe.finish();
    assert_eq!(
        probe.output,
        native(
            "mutations",
            include_str!("../../tests/delegate-owners/mutations.txt")
        )
        .expect("native receipt")
    );
}
