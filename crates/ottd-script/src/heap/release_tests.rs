//! Original free events distinguish GC's immediate node release from a deferred batch.
use crate::{Realm, Value};

#[test]
fn gc_retires_each_table_child_before_clearing_the_next_node() {
    let realm = Realm::new();
    let a = realm.table().expect("A");
    let b = realm.table().expect("B");
    let o = realm.table().expect("O");
    let identities = [
        ('A', a.0.edge.life.id),
        ('B', b.0.edge.life.id),
        ('O', o.0.edge.life.id),
    ];
    o.new_slot(Value::Integer(0), Value::Table(a))
        .expect("A edge");
    o.new_slot(Value::Integer(1), Value::Table(b))
        .expect("B edge");
    o.new_slot(Value::Integer(2), Value::Table(o.clone()))
        .expect("self edge");
    *realm.heap.retired.borrow_mut() = Some(Vec::with_capacity(16));
    drop(o);
    assert_eq!(realm.heap.collect(), 1);
    let trace = realm.heap.retired.borrow();
    let labels = trace
        .as_ref()
        .expect("recording")
        .iter()
        .map(|id| {
            identities
                .iter()
                .find(|(_, identity)| identity == id)
                .expect("registered generation")
                .0
        })
        .collect::<Vec<_>>();
    let native = std::env::var("OTTD_SCRIPT_GC_OWN").ok().map_or_else(
        || include_str!("../../tests/delegate-owners/gc-own.txt").to_owned(),
        |path| std::fs::read_to_string(path).expect("native output"),
    );
    let expected = native
        .lines()
        .filter_map(|line| line.strip_prefix("free "))
        .map(|event| {
            let mut fields = event.split_whitespace();
            let label = fields.next().expect("native identity");
            assert_eq!(label.len(), 1);
            assert_eq!(
                fields.next(),
                Some("88"),
                "pinned native SQTable allocation size"
            );
            assert_eq!(fields.next(), None);
            label.chars().next().expect("label")
        })
        .collect::<Vec<_>>();
    assert_eq!(labels, expected);
}
