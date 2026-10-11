//! Exception objects, caught failures and Rust-only refusal boundaries.
use ottd_script::{
    Execution, Instruction, Program, Realm, Runner, RunnerFailure, Value, Vm, VmError,
};

#[test]
fn arbitrary_uncaught_values_remain_typed_objects() {
    let realm = Realm::new();
    for (expression, expected) in [
        ("null", Value::Null),
        ("true", Value::Bool(true)),
        ("42", Value::Integer(42)),
        ("1.25", Value::Float(1.25_f32.to_bits())),
        ("\"a\\x00Z\"", Value::String(realm.string(b"a\0Z"))),
    ] {
        let mut runner = Runner::new(realm.default_root().expect("root"));
        let program = runner
            .compile(&format!("throw {expression};"))
            .expect("throw source");
        let mut vm = runner.start(&program).expect("frame");
        assert_eq!(vm.resume(0), Ok(Execution::Suspended));
        assert_eq!(vm.resume(10000), Err(VmError::ThrownValue));
        assert_eq!(vm.native_error_value(), &expected);
        assert_eq!(vm.resume(1), Err(VmError::Finished));
        drop(vm);
        assert_eq!(runner.native_error_value(), &expected);
        assert_eq!(
            runner.last_failure(),
            Some(RunnerFailure::Runtime(VmError::ThrownValue))
        );
        runner.reset_native_error();
        assert_eq!(runner.native_error_value(), &Value::Null);
    }
}

#[test]
fn nested_callee_failure_is_caught_without_terminal_failure_history() {
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    let mut runner = Runner::new(root.clone());
    let source = "held<-[7];\nlocal f=function(...){throw vargv[0];};\ntry{try{f(held,2); }catch(inner){inner[0]=9;\nthrow inner;}}catch(outer){return outer==held;}";
    let program = runner.compile(source).expect("nested trap source");
    let mut vm = runner.start(&program).expect("frame");
    let mut outcome = Ok(Execution::Suspended);
    for _ in 0..100 {
        outcome = vm.resume(2);
        if outcome != Ok(Execution::Suspended) {
            break;
        }
    }
    assert_eq!(outcome, Ok(Execution::Returned(Value::Bool(true))));
    assert_eq!(vm.vararg_state().0.len(), 0);
    drop(vm);
    assert_eq!(runner.last_failure(), None);
    assert_eq!(
        runner.native_error_value(),
        &root.raw_get(b"held").expect("same identity")
    );
    let Value::Array(array) = runner.native_error_value() else {
        panic!("array throw");
    };
    assert_eq!(array.get(0), Some(Value::Integer(9)));
}

#[test]
fn retained_history_does_not_make_policy_failures_catchable() {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let prior = runner.compile("assert(false);").expect("prior error");
    assert_eq!(
        runner.start(&prior).expect("prior frame").resume(10000),
        Err(VmError::NativeCall)
    );
    // Rust-only undefined-native boundary: never run this arithmetic in the original VM.
    let program = runner
        .compile("try {return 9223372036854775807+1;} catch(e){return 99;}")
        .expect("checked overflow source");
    assert_eq!(
        runner.start(&program).expect("frame").resume(10000),
        Err(VmError::UnsupportedOverflow)
    );
    assert_eq!(
        runner.native_error().expect("retained history").as_bytes(),
        b"assertion failed"
    );
}

#[test]
fn successful_ignored_set_error_does_not_enter_catch() {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let source = "const C=3;\nlocal f=function(){this=7;\ntry{return this.C+=1;}catch(e){return 99;}};\nreturn f();";
    let program = runner.compile(source).expect("source");
    assert_eq!(
        runner.start(&program).expect("frame").resume(10000),
        Ok(Execution::Returned(Value::Integer(4)))
    );
    assert_eq!(
        runner
            .native_error()
            .expect("source retains ignored error")
            .as_bytes(),
        b"trying to set 'integer'"
    );
    assert_eq!(runner.last_failure(), None);
}

#[test]
fn malformed_trap_operands_are_uncatchable_rust_guards() {
    for instruction in [
        Instruction {
            opcode: 0x38,
            arg0: 2,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        },
        Instruction {
            opcode: 0x38,
            arg0: 1,
            arg1: -2,
            arg2: 0,
            arg3: 0,
        },
        Instruction {
            opcode: 0x39,
            arg0: 1,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        },
        Instruction {
            opcode: 0x3a,
            arg0: 1,
            arg1: 1,
            arg2: 0,
            arg3: 0,
        },
    ] {
        let program = Program::from_parts(2, vec![], vec![instruction]).expect("bounded bytecode");
        let mut vm = Vm::new(&program).expect("frame");
        assert_eq!(vm.resume(100), Err(VmError::InvalidBytecode));
        assert_eq!(vm.native_error_value(), &Value::Null);
    }
}
