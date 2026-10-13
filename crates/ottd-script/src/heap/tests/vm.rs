//! Actual suspended and failing VM graph owners, paired with `graph_vm_session`.
use crate::{Execution, Realm, Runner, Value, VmError};
use std::fmt::Write;
fn pair<A, B>(
    output: &mut String,
    stage: &str,
    array: impl Fn() -> Option<A>,
    leaf: impl Fn() -> Option<B>,
) {
    writeln!(
        output,
        "{stage} {} {}",
        u8::from(array().is_some()),
        u8::from(leaf().is_some())
    )
    .expect("format");
}
fn run(runner: &mut Runner, source: &str) -> Result<Execution, VmError> {
    let program = runner.compile(source).expect("source");
    runner.start(&program)?.resume(10000)
}
#[test]
fn suspended_returned_and_late_error_graphs_match_native_owners() {
    let realm = Realm::new();
    let root = realm.empty_root();
    let mut runner = Runner::new(root.clone());
    let leaf = realm.table().expect("leaf");
    leaf.new_slot(Value::String(realm.string(b"key")), Value::Integer(7))
        .expect("slot");
    let array = realm
        .array(vec![Value::Table(leaf.clone())])
        .expect("array");
    root.new_slot(b"held", Value::Array(array.clone()))
        .expect("root");
    let leaf_watch = leaf.watch();
    let array_watch = array.watch();
    let program = runner
        .compile(include_str!(
            "../../../../../scripts/compat/script-vm/tables/fixtures/graph/suspended.nut"
        ))
        .expect("source");
    let mut vm = runner.start(&program).expect("frame");
    assert_eq!(vm.resume(2), Ok(Execution::Suspended));
    let mut output = String::new();
    writeln!(
        output,
        "suspended {} {}",
        vm.remaining_ops(),
        vm.instruction_pointer()
    )
    .expect("format");
    root.set_existing(b"held", Value::Null).expect("unbind");
    drop(leaf);
    drop(array);
    realm.collect_cycles();
    pair(&mut output, "frame-held", &array_watch, &leaf_watch);
    let Execution::Returned(Value::Array(returned)) = vm.resume(100).expect("return") else {
        panic!("array")
    };
    writeln!(output, "returned {}", vm.remaining_ops()).expect("format");
    drop(vm);
    drop(program);
    run(&mut runner, "return 1;").expect("replace temporary");
    realm.collect_cycles();
    pair(&mut output, "host-result-held", &array_watch, &leaf_watch);
    drop(returned);
    pair(&mut output, "result-released", &array_watch, &leaf_watch);
    let program = runner
        .compile(include_str!(
            "../../../../../scripts/compat/script-vm/tables/fixtures/graph/late-error.nut"
        ))
        .expect("source");
    let mut vm = runner.start(&program).expect("frame");
    let result = vm.resume(10000);
    assert_eq!(result, Err(VmError::MissingIndex));
    writeln!(
        output,
        "late-error {} {}",
        u8::from(result.is_ok()),
        vm.remaining_ops()
    )
    .expect("format");
    drop(vm);
    let Value::Array(array) = root.raw_get(b"held").expect("root") else {
        panic!("array")
    };
    let Some(Value::Table(leaf)) = array.get(0) else {
        panic!("table")
    };
    let array_watch = array.watch();
    let leaf_watch = leaf.watch();
    let Some(Value::Integer(value)) = leaf
        .raw_get(Value::String(realm.string(b"key")))
        .expect("lookup")
    else {
        panic!("integer")
    };
    writeln!(output, "late-value integer {value}").expect("format");
    drop(array);
    drop(leaf);
    realm.collect_cycles();
    pair(&mut output, "late-root-held", &array_watch, &leaf_watch);
    root.set_existing(b"held", Value::Null).expect("unbind");
    pair(&mut output, "late-root-cleared", &array_watch, &leaf_watch);
    run(&mut runner, "return 1;").expect("replace temporary");
    pair(&mut output, "late-temp-replaced", &array_watch, &leaf_watch);
    let native = std::env::var("OTTD_SCRIPT_GRAPH_VM").ok().map_or_else(
        || include_str!("../../../tests/graph-vm.txt").to_owned(),
        |path| std::fs::read_to_string(path).expect("native trace"),
    );
    assert_eq!(output, native);
}
