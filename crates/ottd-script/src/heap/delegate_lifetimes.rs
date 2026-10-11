//! Delegate graph roots cross closure captures, preserved defaults and retained errors.
use super::owner_probe::{Probe, native};
use crate::{Execution, Realm, Runner, Temporary, Value};
use std::fmt::Write;

#[test]
fn captures_defaults_and_last_error_follow_original_collection_lifetimes() {
    for (kind, source, expected) in [
        (
            "capture",
            include_str!("../../tests/delegate-lifetimes/capture.nut"),
            include_str!("../../tests/delegate-lifetimes/capture.txt"),
        ),
        (
            "default",
            include_str!("../../tests/delegate-lifetimes/default.nut"),
            include_str!("../../tests/delegate-lifetimes/default.txt"),
        ),
        (
            "error",
            include_str!("../../tests/delegate-lifetimes/error.nut"),
            include_str!("../../tests/delegate-lifetimes/error.txt"),
        ),
    ] {
        let realm = Realm::new();
        let mut runner = Runner::new(realm.default_root().expect("root"));
        let program = runner
            .compile_bytes_named(source.as_bytes(), c"delegate-owner")
            .expect("source");
        let mut vm = runner.start(&program).expect("frame");
        let result = vm.resume(10000).expect("return");
        let Execution::Returned(Value::Table(table)) = &result else {
            panic!("table result");
        };
        let mut probe = Probe::new(&realm.heap);
        let owner_id = table.0.edge.life.id;
        probe.register('O', table);
        assert!(probe.register_parent('D', table));
        probe.snapshot("result retained").expect("snapshot");
        drop(vm);
        drop(program);
        drop(result);
        runner.temporary = Temporary::Value(Value::Null);
        if kind == "error" {
            let Value::Table(error) = runner.native_error_value() else {
                panic!("table error root");
            };
            assert_eq!(error.0.edge.life.id, owner_id);
            writeln!(
                probe.output,
                "collected error retained {}",
                realm.heap.collect()
            )
            .expect("count");
            probe.snapshot("error retained").expect("snapshot");
            probe.chain().expect("chain");
            runner.reset_native_error();
        }
        writeln!(probe.output, "collected released {}", realm.heap.collect()).expect("count");
        probe.snapshot("released").expect("snapshot");
        probe.finish();
        assert_eq!(
            probe.output,
            native(kind, expected).expect("native lifecycle"),
            "{kind}"
        );
    }
}
