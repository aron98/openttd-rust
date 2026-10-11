//! Native host-entry ownership is measured from actual shared closure/prototype owners.
use crate::{Execution, HostStack, Realm, Runner, Temporary, Value};
use std::{fmt::Write, rc::Rc};
#[test]
fn host_entries_preserve_original_call_owner_and_argument_lifetimes() {
    for retained in [false, true] {
        let realm = Realm::new();
        let root = realm.default_root().expect("root");
        let held = realm.table().expect("held");
        let watch = held.watch();
        root.new_slot(b"held", Value::Table(held)).expect("slot");
        let mut runner = Runner::new(root.clone());
        let program = runner.compile("type(held);\nreturn 0;").expect("source");
        let closure_watch = Rc::downgrade(&program.data);
        let prototype_watch = Rc::downgrade(&program.data.prototype);
        let host = HostStack::new();
        if retained {
            host.push_main(&program);
        }
        let mut output = String::new();
        writeln!(
            output,
            "before {} {} {}",
            Rc::strong_count(&program.data),
            Rc::strong_count(&program.data.prototype),
            host.len()
        )
        .expect("format");
        let mut vm = runner.start_with_host(&program, host.clone()).expect("vm");
        assert_eq!(vm.resume(4), Ok(Execution::Suspended));
        writeln!(
            output,
            "suspended {} {} {} {} {} {}",
            vm.remaining_ops(),
            vm.instruction_pointer(),
            vm.origin,
            vm.origin + vm.registers.len(),
            Rc::strong_count(&program.data),
            Rc::strong_count(&program.data.prototype)
        )
        .expect("format");
        match &vm.runner.get().temporary {
            Temporary::Value(Value::NativeClosure(_)) => {
                writeln!(output, "temporary 134218240").expect("format");
            }
            other => panic!("wrong actual temporary: {other:?}"),
        }
        drop(root.delete_slot(b"held").expect("delete"));
        writeln!(
            output,
            "argument after unbind {}",
            u8::from(watch().is_some())
        )
        .expect("format");
        assert_eq!(vm.resume(100), Ok(Execution::Returned(Value::Integer(0))));
        writeln!(
            output,
            "returned {} {} {} {}",
            vm.remaining_ops(),
            Rc::strong_count(&program.data),
            Rc::strong_count(&program.data.prototype),
            u8::from(watch().is_some())
        )
        .expect("format");
        drop(vm);
        drop(program);
        writeln!(
            output,
            "local released {} {}",
            u8::from(closure_watch.upgrade().is_some()),
            u8::from(prototype_watch.upgrade().is_some())
        )
        .expect("format");
        host.clear();
        writeln!(
            output,
            "host cleared {} {}",
            u8::from(closure_watch.upgrade().is_some()),
            u8::from(prototype_watch.upgrade().is_some())
        )
        .expect("format");
        let expected = if retained {
            include_str!("native-calls-retained.txt")
        } else {
            include_str!("native-calls-empty.txt")
        };
        let variable = if retained {
            "OTTD_SCRIPT_CALL_RETAINED"
        } else {
            "OTTD_SCRIPT_CALL_EMPTY"
        };
        let expected = std::env::var_os(variable).map_or_else(
            || expected.to_owned(),
            |file| std::fs::read_to_string(file).expect("native owner trace"),
        );
        assert_eq!(output, expected);
    }
}

#[test]
fn returned_native_arrays_retain_fill_aliases_until_the_last_temporary_releases() {
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
            || include_str!("native-calls-array.txt").to_owned(),
            |file| std::fs::read_to_string(file).expect("native trace"),
        );
        assert_eq!(output, expected);
    }
}

#[test]
fn invalid_call_destination_preserves_prior_native_print_side_effects() {
    #[derive(Debug)]
    struct Sink(Rc<std::cell::RefCell<Vec<u8>>>);
    impl crate::PrintSink for Sink {
        fn print(&mut self, bytes: &[u8]) {
            self.0.borrow_mut().extend_from_slice(bytes);
        }
    }
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let bytes = Rc::new(std::cell::RefCell::new(Vec::new()));
    runner.set_print_sink(Some(Box::new(Sink(Rc::clone(&bytes)))));
    let source = runner
        .compile("print(\"before\\0after\");")
        .expect("source");
    let mut instructions = source.instructions().to_vec();
    instructions
        .iter_mut()
        .find(|i| i.opcode == 6)
        .expect("call")
        .arg0 = 254;
    let program = crate::Program::freeze(crate::ProgramData {
        stack_size: source.stack_size(),
        literals: source.literals().to_vec(),
        instructions,
        realm,
    });
    assert_eq!(
        runner.start(&program).expect("vm").resume(100),
        Err(crate::VmError::InvalidBytecode)
    );
    assert_eq!(bytes.borrow().as_slice(), b"before\0after");
}

#[test]
fn foreign_host_entries_fail_before_a_main_frame_is_installed() {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let program = runner.compile("return type(1);").expect("source");
    let foreign = crate::compile("return 7;").expect("foreign");
    let host = HostStack::new();
    host.push_main(&foreign);
    assert!(matches!(
        runner.start_with_host(&program, host),
        Err(crate::VmError::RealmMismatch)
    ));
    assert!(matches!(runner.temporary, Temporary::Value(Value::Null)));
}

#[test]
fn malformed_native_argument_ranges_fail_without_dispatching_the_body() {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let source = runner.compile("return array(2);").expect("source");
    for base in [254, 255] {
        let mut instructions = source.instructions().to_vec();
        instructions
            .iter_mut()
            .find(|i| i.opcode == 6)
            .expect("call")
            .arg2 = base;
        let program = crate::Program::freeze(crate::ProgramData {
            stack_size: source.stack_size(),
            literals: source.literals().to_vec(),
            instructions,
            realm: realm.clone(),
        });
        assert_eq!(
            runner.start(&program).expect("vm").resume(100),
            Err(crate::VmError::InvalidBytecode)
        );
        assert!(runner.native_error().is_none());
    }
}

#[test]
fn native_failure_preserves_aliases_mutation_and_reusable_runner_state() {
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    let mut runner = Runner::new(root.clone());
    let source =
        include_str!("../../../../scripts/compat/script-vm/tables/fixtures/calls/late-error.nut");
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
        || include_str!("native-calls-failure.txt").to_owned(),
        |file| std::fs::read_to_string(file).expect("native trace"),
    );
    assert_eq!(output, expected);
}

#[test]
fn call_created_cycles_release_on_realm_teardown() {
    let mut output = String::new();
    let (table_watch, array_watch) = {
        let realm = Realm::new();
        let mut runner = Runner::new(realm.default_root().expect("root"));
        let program = runner
            .compile("local t={};\nlocal a=array(2,t);\nt.a<-a;\nreturn a;")
            .expect("source");
        let mut vm = runner.start(&program).expect("vm");
        let Execution::Returned(Value::Array(array)) = vm.resume(10000).expect("returned") else {
            panic!("array");
        };
        let Value::Table(table) = array.get(0).expect("fill") else {
            panic!("table");
        };
        writeln!(
            output,
            "cycle returned {} {}",
            vm.remaining_ops(),
            u8::from(array.get(1) == Some(Value::Table(table.clone())))
        )
        .expect("format");
        let watches = (table.watch(), array.watch());
        drop(table);
        drop(array);
        vm.runner.get_mut().temporary = Temporary::Value(Value::Null);
        writeln!(
            output,
            "cycle before {} {}",
            u8::from(watches.0().is_some()),
            u8::from(watches.1().is_some())
        )
        .expect("format");
        watches
    };
    writeln!(
        output,
        "cycle after {} {}",
        u8::from(table_watch().is_some()),
        u8::from(array_watch().is_some())
    )
    .expect("format");
    let expected = std::env::var_os("OTTD_SCRIPT_CALL_CYCLE").map_or_else(
        || include_str!("native-calls-cycle.txt").to_owned(),
        |file| std::fs::read_to_string(file).expect("native trace"),
    );
    assert_eq!(output, expected);
}

#[test]
fn unsupported_native_conversions_remain_explicit_after_argument_evaluation() {
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    let mut runner = Runner::new(root.clone());
    for bits in [
        f32::NAN.to_bits(),
        f32::INFINITY.to_bits(),
        f32::NEG_INFINITY.to_bits(),
    ] {
        root.new_slot(b"size", Value::Float(bits)).expect("size");
        let program = runner.compile("return array(size);").expect("source");
        assert_eq!(
            runner.start(&program).expect("vm").resume(100),
            Err(crate::VmError::UnsupportedIndexConversion)
        );
    }
    let program = runner
        .compile("held<-{};\nreturn print(held);")
        .expect("source");
    assert_eq!(
        runner.start(&program).expect("vm").resume(100),
        Err(crate::VmError::UnsupportedRuntimeValue)
    );
    assert!(matches!(root.raw_get(b"held"), Some(Value::Table(_))));
}

#[test]
fn call_temporary_retains_the_exact_unbound_callee_until_return() {
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    let Value::NativeClosure(callee) = root.raw_get(b"type").expect("type") else {
        panic!("native closure");
    };
    let watch = callee.watch();
    drop(callee);
    let mut runner = Runner::new(root.clone());
    let program = runner.compile("type(7);\nreturn 0;").expect("source");
    let mut vm = runner.start(&program).expect("vm");
    assert_eq!(vm.resume(4), Ok(Execution::Suspended));
    let Temporary::Value(Value::NativeClosure(actual)) = &vm.runner.get().temporary else {
        panic!("callee temporary");
    };
    let mut output = format!(
        "callee identity {}\n",
        u8::from(watch().is_some_and(|expected| actual.same_identity(&expected)))
    );
    drop(root.delete_slot(b"type").expect("delete callee"));
    writeln!(output, "callee unbound {}", u8::from(watch().is_some())).expect("format");
    assert_eq!(vm.resume(100), Ok(Execution::Returned(Value::Integer(0))));
    writeln!(
        output,
        "callee returned {} {}",
        vm.remaining_ops(),
        u8::from(watch().is_some())
    )
    .expect("format");
    let expected = std::env::var_os("OTTD_SCRIPT_CALL_CALLEE").map_or_else(
        || include_str!("native-calls-callee.txt").to_owned(),
        |file| std::fs::read_to_string(file).expect("native trace"),
    );
    assert_eq!(output, expected);
}
