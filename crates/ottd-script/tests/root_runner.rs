//! Persistent configured roots use the public Runner API and the production VM.
use ottd_script::{CompileErrorKind, Execution, Realm, Runner, RunnerFailure, Value, VmError};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[test]
fn shared_root_and_independent_runner_state_survive_errors() -> Result<()> {
    let realm = Realm::new();
    let root = realm.empty_root();
    let mut first = Runner::new(root.clone());
    let mut second = Runner::new(root.clone());
    let program = first.compile("score <- 7; return absent;")?;
    assert_eq!(
        first.start(&program)?.resume(100),
        Err(VmError::MissingIndex)
    );
    assert_eq!(root.raw_get(b"score"), Some(Value::Integer(7)));
    assert_eq!(
        first.last_failure(),
        Some(RunnerFailure::Runtime(VmError::MissingIndex))
    );
    assert_eq!(second.last_failure(), None);
    let read = second.compile("return score;")?;
    assert_eq!(
        second.start(&read)?.resume(100)?,
        Execution::Returned(Value::Integer(7))
    );
    first.replace_root(realm.empty_root())?;
    assert_eq!(first.start(&read)?.resume(100), Err(VmError::MissingIndex));
    assert_eq!(
        second.start(&read)?.resume(100)?,
        Execution::Returned(Value::Integer(7))
    );
    Ok(())
}
#[test]
fn root_replacement_is_run_time_but_constants_are_compile_time() -> Result<()> {
    let realm = Realm::new();
    let original = realm.empty_root();
    original.new_slot(b"score", Value::Integer(1))?;
    let mut runner = Runner::new(original.clone());
    let explicit = runner.compile("return ::score;")?;
    let unresolved = runner.compile("return score;")?;
    let constant = runner.compile("const score=9;\nreturn score;")?;
    let replacement = realm.empty_root();
    replacement.new_slot(b"score", Value::Integer(2))?;
    runner.replace_root(replacement)?;
    for (program, answer) in [(&explicit, 2), (&unresolved, 2), (&constant, 9)] {
        assert_eq!(
            runner.start(program)?.resume(100)?,
            Execution::Returned(Value::Integer(answer))
        );
    }
    assert_eq!(original.raw_get(b"score"), Some(Value::Integer(1)));
    Ok(())
}
#[test]
fn supported_lookup_errors_do_not_conflate_native_unsupported_values() -> Result<()> {
    let realm = Realm::new();
    let root = realm.empty_root();
    let mut runner = Runner::new(root.clone());
    let delegate = runner.compile("return len;")?;
    runner.compile("const len=4;")?;
    assert_eq!(
        runner.start(&delegate)?.resume(100),
        Err(VmError::UnsupportedRuntimeValue)
    );
    root.new_slot(b"len", Value::Integer(3))?;
    assert_eq!(
        runner.start(&delegate)?.resume(100)?,
        Execution::Returned(Value::Integer(3))
    );
    let late = runner.compile("return Later;")?;
    runner.compile("enum Later { X=12 }")?;
    assert_eq!(
        runner.start(&late)?.resume(100),
        Err(VmError::UnsupportedRuntimeValue)
    );
    let missing = runner.compile("return absent;")?;
    assert_eq!(
        runner.start(&missing)?.resume(100),
        Err(VmError::MissingIndex)
    );
    Ok(())
}
#[test]
fn host_preconditions_and_scalar_results_have_separate_ownership() -> Result<()> {
    let realm = Realm::new();
    let root = realm.empty_root();
    let mut runner = Runner::new(root.clone());
    let program = runner.compile("score <- \"owned\"; return score;")?;
    assert_eq!(
        root.set_existing(b"absent", Value::Null),
        Err(VmError::MissingIndex)
    );
    assert_eq!(root.raw_get(b"absent"), None);
    let foreign = Realm::new().compile("return 1;")?;
    assert!(matches!(
        runner.start(&foreign),
        Err(VmError::RealmMismatch)
    ));
    assert_eq!(
        runner.replace_root(Realm::new().empty_root()),
        Err(VmError::RealmMismatch)
    );
    assert_eq!(runner.last_failure(), None);
    let result = runner.start(&program)?.resume(100)?;
    root.new_slot(b"score", Value::Null)?;
    drop(program);
    drop(runner);
    drop(root);
    drop(realm);
    assert!(
        matches!(result, Execution::Returned(Value::String(bytes)) if bytes.as_bytes() == b"owned")
    );
    Ok(())
}
#[test]
fn unsupported_root_source_remains_distinct_from_lexer_policy() {
    for source in [
        "return this;",
        "return ::score.x;",
        "score += 1;",
        "++score;",
        "score++;",
    ] {
        let error = Realm::new()
            .compile(source)
            .expect_err("outside the scalar root domain");
        assert_eq!(error.kind, CompileErrorKind::UnsupportedSyntax, "{source}");
    }
}
#[test]
fn missing_out_of_scope_local_is_a_runtime_lookup() -> Result<()> {
    let program = Realm::new().compile("local x=1; {local y=2;} return y;")?;
    assert_eq!(
        ottd_script::Vm::new(&program)?.resume(100),
        Err(VmError::MissingIndex)
    );
    Ok(())
}
#[test]
fn checked_bytecode_keeps_root_identity_separate_from_scalar_operands() -> Result<()> {
    use ottd_script::{Instruction, Program, Vm};
    let realm = Realm::new();
    for (instruction, literals, expected) in [
        (
            Instruction {
                opcode: 0x15,
                arg0: 255,
                arg1: 0,
                arg2: 0,
                arg3: 0,
            },
            vec![],
            VmError::InvalidBytecode,
        ),
        (
            Instruction {
                opcode: 0x09,
                arg0: 1,
                arg1: 0,
                arg2: 0,
                arg3: 0,
            },
            vec![Value::Integer(1)],
            VmError::UnsupportedRuntimeValue,
        ),
        (
            Instruction {
                opcode: 0x09,
                arg0: 1,
                arg1: 0,
                arg2: 1,
                arg3: 0,
            },
            vec![Value::String(realm.string(b"key"))],
            VmError::UnsupportedReceiver,
        ),
        (
            Instruction {
                opcode: 0x09,
                arg0: 1,
                arg1: 8,
                arg2: 0,
                arg3: 0,
            },
            vec![],
            VmError::InvalidBytecode,
        ),
    ] {
        let program = Program::from_parts(2, literals, vec![instruction])?;
        assert_eq!(Vm::new(&program)?.resume(10), Err(expected));
    }
    Ok(())
}
