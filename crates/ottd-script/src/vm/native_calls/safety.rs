//! Native-call safety consumer scenarios.
use crate::{Execution, HostStack, Realm, Runner, Temporary, Value};
use std::{fmt::Write, rc::Rc};

pub(super) fn invalid_call_destination_preserves_prior_native_print_side_effects() {
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
    let mut instructions = source.instructions();
    instructions
        .iter_mut()
        .find(|i| i.opcode == 6)
        .expect("call")
        .arg0 = 254;
    let program = crate::Program::freeze(crate::ProgramData {
        stack_size: source.stack_size(),
        literals: source.literals(),
        instructions,
        realm,
    });
    assert_eq!(
        runner.start(&program).expect("vm").resume(100),
        Err(crate::VmError::InvalidBytecode)
    );
    assert_eq!(bytes.borrow().as_slice(), b"before\0after");
}

pub(super) fn foreign_host_entries_fail_before_a_main_frame_is_installed() {
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
    assert!(matches!(
        runner.temporary.as_ref(),
        Temporary::Value(Value::Null)
    ));
}

pub(super) fn malformed_native_argument_ranges_fail_without_dispatching_the_body() {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let source = runner.compile("return array(2);").expect("source");
    for base in [254, 255] {
        let mut instructions = source.instructions();
        instructions
            .iter_mut()
            .find(|i| i.opcode == 6)
            .expect("call")
            .arg2 = base;
        let program = crate::Program::freeze(crate::ProgramData {
            stack_size: source.stack_size(),
            literals: source.literals(),
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

pub(super) fn unsupported_native_conversions_remain_explicit_after_argument_evaluation() {
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

pub(super) fn call_temporary_retains_the_exact_unbound_callee_until_return() {
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
    let Temporary::Value(Value::NativeClosure(actual)) = vm.runner.get().temporary.as_ref() else {
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
        || include_str!("../native-calls-callee.txt").to_owned(),
        |file| std::fs::read_to_string(file).expect("native trace"),
    );
    assert_eq!(output, expected);
}
