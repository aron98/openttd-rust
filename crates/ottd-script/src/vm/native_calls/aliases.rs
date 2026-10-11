//! Native-call aliases consumer scenarios.
use crate::{Execution, HostStack, Realm, Runner, Temporary, Value};
use std::fmt::Write;

pub(super) fn returned_native_arrays_retain_fill_aliases_until_the_last_temporary_releases() {
    for retained in [false, true] {
        let realm = Realm::new();
        let root = realm.default_root().expect("root");
        let held = realm.table().expect("held");
        let watch = held.watch();
        root.new_slot(b"held", Value::Table(held)).expect("slot");
        let mut runner = Runner::new(root.clone());
        let program = runner.compile("return array(2,held);").expect("source");
        let host = HostStack::new();
        if retained {
            host.push_main(&program);
        }
        let mut vm = runner.start_with_host(&program, host).expect("vm");
        assert_eq!(vm.resume(5), Ok(Execution::Suspended));
        let mut output = format!(
            "suspended {} {}\n",
            vm.remaining_ops(),
            vm.instruction_pointer()
        );
        let destination = program
            .instructions()
            .iter()
            .find(|i| i.opcode == 6)
            .expect("call")
            .arg0;
        let Value::Array(array) = vm.register(i32::from(destination)).expect("array slot") else {
            panic!("array");
        };
        let array_watch = array.watch();
        writeln!(
            output,
            "fill alias {}",
            u8::from(array.get(0) == array.get(1))
        )
        .expect("format");
        drop(array);
        drop(root.delete_slot(b"held").expect("delete"));
        writeln!(output, "after unbind {}", u8::from(watch().is_some())).expect("format");
        let Execution::Returned(returned) = vm.resume(100).expect("return") else {
            panic!("return");
        };
        writeln!(
            output,
            "returned {} {}",
            vm.remaining_ops(),
            u8::from(matches!(returned, Value::Array(_)))
        )
        .expect("format");
        drop(returned);
        writeln!(
            output,
            "temporary held {} {}",
            u8::from(array_watch().is_some()),
            u8::from(watch().is_some())
        )
        .expect("format");
        vm.runner.get_mut().temporary = Temporary::Value(Value::Null);
        writeln!(
            output,
            "temporary released {} {}",
            u8::from(array_watch().is_some()),
            u8::from(watch().is_some())
        )
        .expect("format");
        let variable = if retained {
            "OTTD_SCRIPT_CALL_ARRAY_RETAINED"
        } else {
            "OTTD_SCRIPT_CALL_ARRAY_EMPTY"
        };
        let expected = std::env::var_os(variable).map_or_else(
            || include_str!("../native-calls-array.txt").to_owned(),
            |file| std::fs::read_to_string(file).expect("native trace"),
        );
        assert_eq!(output, expected);
    }
}

pub(super) fn native_failure_preserves_aliases_mutation_and_reusable_runner_state() {
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    let mut runner = Runner::new(root.clone());
    let source = include_str!(
        "../../../../../scripts/compat/script-vm/tables/fixtures/calls/late-error.nut"
    );
    let program = runner.compile(source).expect("source");
    let mut vm = runner.start(&program).expect("vm");
    assert_eq!(vm.resume(10000), Err(crate::VmError::NativeCall));
    let mut output = format!(
        "failed 0 {} {}\n",
        vm.remaining_ops(),
        std::str::from_utf8(vm.native_error().expect("native error").as_bytes())
            .expect("UTF8 error")
    );
    let Value::Table(held) = root.raw_get(b"held").expect("held") else {
        panic!("held table");
    };
    let Value::Array(copy) = root.raw_get(b"copy").expect("copy") else {
        panic!("copy array");
    };
    let held_watch = held.watch();
    let copy_watch = copy.watch();
    let Value::Integer(found) = held
        .raw_get(Value::String(realm.string(b"key")))
        .expect("key")
        .expect("found")
    else {
        panic!("integer");
    };
    writeln!(
        output,
        "retained {} {} {} {}",
        found,
        copy.len(),
        u8::from(copy.get(0) == Some(Value::Table(held.clone()))),
        u8::from(copy.get(1) == Some(Value::Table(held.clone())))
    )
    .expect("format");
    match &vm.runner.get().temporary {
        Temporary::Value(Value::NativeClosure(_)) => {
            writeln!(output, "temporary 134218240").expect("format");
        }
        other => panic!("actual temporary {other:?}"),
    }
    drop(vm);
    drop(held);
    drop(copy);
    drop(root.delete_slot(b"held").expect("delete held"));
    drop(root.delete_slot(b"copy").expect("delete copy"));
    writeln!(
        output,
        "unbound {} {}",
        u8::from(held_watch().is_some()),
        u8::from(copy_watch().is_some())
    )
    .expect("format");
    let next = runner.compile("return type(7);").expect("next source");
    let mut vm = runner.start(&next).expect("next vm");
    let Execution::Returned(Value::String(result)) = vm.resume(100).expect("next call") else {
        panic!("string return");
    };
    writeln!(
        output,
        "next {} {} {}",
        vm.remaining_ops(),
        std::str::from_utf8(result.as_bytes()).expect("type"),
        std::str::from_utf8(vm.native_error().expect("retained error").as_bytes()).expect("error")
    )
    .expect("format");
    let expected = std::env::var_os("OTTD_SCRIPT_CALL_FAILURE").map_or_else(
        || include_str!("../native-calls-failure.txt").to_owned(),
        |file| std::fs::read_to_string(file).expect("native trace"),
    );
    assert_eq!(output, expected);
}
