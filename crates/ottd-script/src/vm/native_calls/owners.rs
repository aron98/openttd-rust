//! Native-call owners consumer scenarios.
use crate::{Execution, HostStack, Realm, Runner, Temporary, Value};
use std::{fmt::Write, rc::Rc};

#[expect(
    clippy::arithmetic_side_effects,
    reason = "The native trace adds a zero-or-one host prefix to the bounded register count"
)]
pub(super) fn host_entries_preserve_original_call_owner_and_argument_lifetimes() {
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
            include_str!("../native-calls-retained.txt")
        } else {
            include_str!("../native-calls-empty.txt")
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

pub(super) fn call_created_cycles_release_on_realm_teardown() {
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
        || include_str!("../native-calls-cycle.txt").to_owned(),
        |file| std::fs::read_to_string(file).expect("native trace"),
    );
    assert_eq!(output, expected);
}
