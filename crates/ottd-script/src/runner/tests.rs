//! Exact ownership transitions use the same interner observed by native ref probes.
use super::{Runner, Temporary};
use crate::{Execution, Realm, Value, VmError};
use std::rc::Rc;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[test]
fn program_clone_shares_one_literal_pool_and_failed_call_retains_real_program() -> Result<()> {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.empty_root());
    let a = runner.compile("local keep=\"compile-owner-A\"; return absent;")?;
    let clone = a.clone();
    assert!(Rc::ptr_eq(&a.data, &clone.data));
    assert_eq!(realm.owners(b"compile-owner-A"), 1);
    let b = runner.compile("return \"compile-owner-B\";")?;
    assert!(matches!(runner.temporary, Temporary::Value(Value::Null)));
    drop(clone);
    {
        let mut frame = runner.start(&a)?;
        drop(a);
        assert_eq!(frame.resume(0)?, Execution::Suspended);
        drop(b);
        assert_eq!(realm.owners(b"compile-owner-B"), 0);
        assert_eq!(frame.resume(100), Err(VmError::MissingIndex));
        assert_eq!(realm.owners(b"compile-owner-A"), 1);
    }
    let Temporary::MainProgram { _program: owner } = &runner.temporary else {
        return Err("missing actual program owner".into());
    };
    assert_eq!(owner.literals.len(), 2);
    let c = runner.compile("return null;")?;
    assert_eq!(realm.owners(b"compile-owner-A"), 1);
    drop(runner.start(&c)?);
    assert_eq!(realm.owners(b"compile-owner-A"), 1);
    assert_eq!(
        runner.start(&c)?.resume(100)?,
        Execution::Returned(Value::Null)
    );
    assert_eq!(realm.owners(b"compile-owner-A"), 0);
    Ok(())
}
#[test]
fn terminal_frame_releases_program_before_wrapper_drop() -> Result<()> {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.empty_root());
    let program = runner.compile("local unused=\"only-in-prototype\"; return 1;")?;
    let mut frame = runner.start(&program)?;
    drop(program);
    assert_eq!(realm.owners(b"only-in-prototype"), 1);
    assert_eq!(frame.resume(100)?, Execution::Returned(Value::Integer(1)));
    assert_eq!(realm.owners(b"only-in-prototype"), 0);
    assert_eq!(frame.resume(1), Err(VmError::Finished));
    Ok(())
}
#[test]
fn compile_failure_keeps_previous_temporary_and_success_keeps_error_record() -> Result<()> {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.empty_root());
    let program = runner.compile("return \"retained-temp\";")?;
    let result = runner.start(&program)?.resume(100)?;
    drop(result);
    drop(program);
    assert_eq!(realm.owners(b"retained-temp"), 1);
    assert!(runner.compile("const Keep=7;\nlocal = ;").is_err());
    let error = runner.last_failure();
    assert_eq!(realm.owners(b"retained-temp"), 1);
    let next = runner.compile("return Keep;")?;
    assert_eq!(runner.last_failure(), error);
    assert_eq!(
        runner.start(&next)?.resume(100)?,
        Execution::Returned(Value::Integer(7))
    );
    assert_eq!(runner.last_failure(), error);
    assert_eq!(realm.owners(b"retained-temp"), 0);
    Ok(())
}
