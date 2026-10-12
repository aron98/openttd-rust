//! Required success-expecting cases against actual original root observations.
use super::observation_native as native;
use crate::{
    Realm, Runner,
    heap::observation::{HeapSnapshot, ObjectKind, ValueKind},
};

fn tables(snapshot: &HeapSnapshot) -> usize {
    snapshot
        .entries
        .iter()
        .filter(|entry| entry.kind == Some(ObjectKind::Table) && entry.life.is_some())
        .count()
}
#[test]
#[ignore = "Required native shared-state graph; actual original capture directory must be provided"]
fn required_native_shared_table_graph_exists_before_and_after_idle_collection() {
    let original = native::receipt("shared");
    let minimum = native::shared_table_minimum(&original);
    let realm = Realm::new();
    let runner = Runner::new(realm.default_root().expect("ordinary default root"));
    let before = realm.heap.snapshot();
    assert_eq!(
        runner.snapshot().root.kind,
        native::field(&original, "idle", "root").kind
    );
    let count = realm.heap.collect();
    let after = realm.heap.snapshot();
    assert_eq!(count, native::collected(&original, "shared-collected"));
    println!(
        "native distinct known table minimum {minimum}; Rust before {} after {}; snapshots {before:?} {after:?}",
        tables(&before),
        tables(&after)
    );
    assert!(
        tables(&before) >= minimum && tables(&after) >= minimum,
        "ordinary context must retain all actual shared tables, not only its root"
    );
}

#[test]
#[ignore = "Required native friend root/GC reachability; actual original capture directory must be provided"]
fn required_native_local_friend_releases_root_when_collected() {
    let original = native::receipt("friend-local");
    let realm = Realm::new();
    let primary = Runner::new(realm.default_root().expect("ordinary primary root"));
    let friend = primary.friend().expect("ordinary friend");
    let before = friend.snapshot();
    let heap_before = realm.heap.snapshot();
    assert_eq!(
        before.root.kind,
        native::field(&original, "friend-before", "root").kind
    );
    let count = realm.heap.collect();
    let after = friend.snapshot();
    let heap_after = realm.heap.snapshot();
    println!(
        "actual Rust friend before {before:?}, after {after:?}, collected {count}; heap before {heap_before:?}, after {heap_after:?}"
    );
    let expected = native::field(&original, "friend-after", "root").kind;
    assert_eq!(
        (after.root.kind, count),
        (expected, native::collected(&original, "primary-collected")),
        "a genuine local reference must not become an extra native GC root"
    );
    // No claim about native post-release chain traversal or another collection.
    assert_eq!(primary.snapshot().root.kind, ValueKind::Table);
}

#[test]
#[ignore = "Requires actual original persistent error/compile/reset receipts"]
fn native_persistent_payload_snapshots_preserve_the_reached_field_types() {
    use crate::{Execution, Value};
    enum Phase {
        Retained,
        CompileError,
        CompileSuccess,
        Reset,
    }
    let original = native::receipt("persistent");
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let program = runner
        .compile_bytes_named(
            b"try { throw [7]; } catch(e) {} return 1;",
            c"roots-persistent",
        )
        .expect("source");
    let mut vm = runner.start(&program).expect("frame");
    assert_eq!(vm.resume(10000), Ok(Execution::Returned(Value::Integer(1))));
    drop(vm);
    drop(program);
    for action in [
        Phase::Retained,
        Phase::CompileError,
        Phase::CompileSuccess,
        Phase::Reset,
    ] {
        let phase = match action {
            Phase::Retained => "retained",
            Phase::CompileError => {
                assert!(
                    runner
                        .compile_bytes_named(b"local = ;", c"roots-error")
                        .is_err()
                );
                "compile-error"
            }
            Phase::CompileSuccess => {
                drop(
                    runner
                        .compile_bytes_named(b"return 2;", c"roots-success")
                        .expect("compile success"),
                );
                "compile-success"
            }
            Phase::Reset => {
                runner.reset_native_error();
                "reset"
            }
        };
        assert_eq!(realm.heap.collect(), native::collected(&original, phase));
        let snapshot = runner.snapshot();
        println!("phase {phase}: {snapshot:?}");
        for (name, field) in [
            ("root", snapshot.root),
            ("error", snapshot.error),
            ("temporary", snapshot.temporary),
        ] {
            let expected = native::field(&original, phase, name);
            assert_eq!(field.kind, expected.kind);
            if let Some(identity) = field.identity {
                let heap = realm.heap.snapshot();
                let counts = heap
                    .entries
                    .iter()
                    .find(|entry| entry.identity == identity)
                    .expect("actual generation")
                    .life
                    .expect("retained lifetime");
                assert_eq!(counts.references, expected.references);
            }
        }
    }
}
