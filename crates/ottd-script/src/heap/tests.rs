//! Graph consumers are checked against passive original-VM lifetime observations.
use crate::{Execution, Realm, Runner, Value, VmError};
use std::fmt::Write;
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "One ordered native host session covers shared setup, clone and collection transitions"
)]
fn graph_edges_and_collection_match_native_host_handles() {
    let realm = Realm::new();
    let key = Value::String(realm.string(b"key"));
    let leaf = realm.table().expect("leaf");
    leaf.new_slot(key.clone(), Value::String(realm.string(b"graph-leaf")))
        .expect("slot");
    let watch = leaf.watch();
    let array = realm
        .array(vec![Value::Table(leaf.clone())])
        .expect("array");
    let parent = realm.table().expect("parent");
    parent
        .new_slot(key.clone(), Value::Array(array.clone()))
        .expect("slot");
    let copy = parent.shallow_clone().expect("clone");
    let mut output = String::new();
    let Some(Value::Array(found)) = copy.raw_get(key.clone()).expect("lookup") else {
        panic!("array")
    };
    writeln!(
        output,
        "clone alias {}",
        u8::from(found.same_identity(&array))
    )
    .expect("format");
    drop(found);
    drop(leaf);
    drop(array);
    drop(parent);
    writeln!(output, "dag clone-held {}", u8::from(watch().is_some())).expect("format");
    drop(copy);
    writeln!(output, "dag released {}", u8::from(watch().is_some())).expect("format");
    let hidden = realm.table().expect("hidden");
    let leaf = realm.table().expect("leaf");
    let watch = leaf.watch();
    hidden
        .set_existing(Value::Null, Value::Table(leaf))
        .expect("hidden edge");
    let copy = hidden.shallow_clone().expect("copy");
    writeln!(output, "hidden held {}", u8::from(watch().is_some())).expect("format");
    drop(hidden);
    writeln!(output, "hidden clone-omits {}", u8::from(watch().is_some())).expect("format");
    drop(copy);
    let cycle = realm.table().expect("cycle");
    let watch = cycle.watch();
    cycle
        .new_slot(key.clone(), Value::Table(cycle.clone()))
        .expect("cycle edge");
    let collected = realm.heap.collect();
    writeln!(
        output,
        "cycle rooted-collect {collected} {}",
        u8::from(watch().is_some())
    )
    .expect("format");
    drop(cycle);
    writeln!(
        output,
        "cycle unrooted-before {}",
        u8::from(watch().is_some())
    )
    .expect("format");
    let collected = realm.heap.collect();
    writeln!(
        output,
        "cycle collected {collected} {}",
        u8::from(watch().is_some())
    )
    .expect("format");
    let array = realm.array(vec![]).expect("array");
    let parent = realm.table().expect("parent");
    let array_watch = array.watch();
    let watch = parent.watch();
    array
        .append(Value::Table(parent.clone()))
        .expect("array edge");
    parent
        .new_slot(key, Value::Array(array.clone()))
        .expect("table edge");
    drop(parent);
    let collected = realm.heap.collect();
    writeln!(
        output,
        "pair rooted-collect {collected} {} {}",
        u8::from(watch().is_some()),
        u8::from(array_watch().is_some())
    )
    .expect("format");
    drop(array);
    writeln!(
        output,
        "pair unrooted-before {} {}",
        u8::from(watch().is_some()),
        u8::from(array_watch().is_some())
    )
    .expect("format");
    let collected = realm.heap.collect();
    writeln!(
        output,
        "pair collected {collected} {} {}",
        u8::from(watch().is_some()),
        u8::from(array_watch().is_some())
    )
    .expect("format");
    let leaf = realm.table().expect("leaf");
    let watch = leaf.watch();
    let mut chain = Value::Table(leaf);
    for _ in 0..2048 {
        chain = Value::Array(realm.array(vec![chain]).expect("chain"));
    }
    writeln!(output, "deep held {}", u8::from(watch().is_some())).expect("format");
    drop(chain);
    writeln!(output, "deep released {}", u8::from(watch().is_some())).expect("format");
    let native = std::env::var("OTTD_SCRIPT_GRAPH_STORAGE").ok().map_or_else(
        || include_str!("../../tests/graph-storage.txt").to_owned(),
        |path| std::fs::read_to_string(path).expect("native output"),
    );
    assert_eq!(output, native);
}
#[test]
fn suspended_frame_and_returned_graph_are_live_collection_roots() {
    let realm = Realm::new();
    let root = realm.empty_root();
    let mut runner = Runner::new(root.clone());
    let program = runner
        .compile("held <- [{key=7}];\nlocal alias=held;\nreturn alias;")
        .expect("program");
    let mut vm = runner.start(&program).expect("frame");
    while root.raw_get(b"held").is_none() {
        assert_eq!(vm.resume(2).expect("step"), Execution::Suspended);
    }
    let Value::Array(array) = root.raw_get(b"held").expect("held") else {
        panic!("array")
    };
    let watch = array.watch();
    drop(array);
    root.set_existing(b"held", Value::Null).expect("unbind");
    realm.collect_cycles();
    assert!(watch().is_some());
    // The local alias may not have executed before host rebinding; the object
    // remains owned by the suspended frame regardless of the eventual result.
    let _result = vm.resume(100).expect("complete");
    drop(vm);
    let next = runner.compile("return 1;").expect("next");
    runner
        .start(&next)
        .expect("frame")
        .resume(100)
        .expect("result");
    assert!(watch().is_none());
}
#[test]
fn graph_affinity_and_failure_order_are_preserved() {
    let realm = Realm::new();
    let foreign = Realm::new().table().expect("foreign");
    let table = realm.table().expect("table");
    assert_eq!(
        table.new_slot(Value::Null, Value::Table(foreign.clone())),
        Err(VmError::NullIndex)
    );
    assert_eq!(
        table.set_existing(Value::Integer(0), Value::Table(foreign.clone())),
        Err(VmError::MissingIndex)
    );
    assert_eq!(
        table.new_slot(Value::Integer(0), Value::Table(foreign.clone())),
        Err(VmError::RealmMismatch)
    );
    let array = realm.array(vec![Value::Null]).expect("array");
    assert_eq!(
        array.set(1, Value::Table(foreign.clone())),
        Err(VmError::MissingIndex)
    );
    assert_eq!(
        array.set(0, Value::Table(foreign)),
        Err(VmError::RealmMismatch)
    );
    assert_eq!(array.get(0), Some(Value::Null));
}
#[test]
fn dead_generation_cannot_be_resurrected_after_slot_reuse() {
    let realm = Realm::new();
    let first = realm.table().expect("first");
    let watch = first.watch();
    drop(first);
    let second = realm.table().expect("second");
    assert!(watch().is_none());
    assert!(second.is_empty());
}
#[test]
fn realm_teardown_releases_internal_cycles_without_strong_heap_edges() {
    let realm = Realm::new();
    let pool = Rc::downgrade(&realm.heap);
    let table = realm.table().expect("table");
    let watch = table.watch();
    let array = realm
        .array(vec![Value::Table(table.clone())])
        .expect("array");
    let array_watch = array.watch();
    table
        .new_slot(Value::Integer(1), Value::Array(array.clone()))
        .expect("cycle");
    drop(table);
    drop(array);
    let mut output = format!(
        "teardown before {} {}\n",
        u8::from(watch().is_some()),
        u8::from(array_watch().is_some())
    );
    drop(realm);
    assert!(pool.upgrade().is_none());
    writeln!(
        output,
        "teardown after {} {}",
        u8::from(watch().is_some()),
        u8::from(array_watch().is_some())
    )
    .expect("format");
    let native = std::env::var("OTTD_SCRIPT_GRAPH_TEARDOWN")
        .ok()
        .map_or_else(
            || include_str!("../../tests/graph-teardown.txt").to_owned(),
            |path| std::fs::read_to_string(path).expect("native output"),
        );
    assert_eq!(output, native);
}
use std::rc::Rc;

mod vm;
