//! Existing-API regressions captured failing before array implementation.
use ottd_script::{Execution, Realm, Runner, Value, Vm, compile};
use std::error::Error;

type Result = std::result::Result<(), Box<dyn Error>>;

#[test]
fn production_array_allocation_and_index_mutation() -> Result {
    let program = compile("local a=[1,2]; a[0]=7; return a[0];")?;
    assert_eq!(
        Vm::new(&program)?.resume(1000)?,
        Execution::Returned(Value::Integer(7))
    );
    Ok(())
}

#[test]
fn production_array_persists_between_runner_programs() -> Result {
    let mut runner = Runner::new(Realm::new().empty_root());
    let initialize = runner.compile("pending <- [1,2]; return 0;")?;
    assert_eq!(
        runner.start(&initialize)?.resume(1000)?,
        Execution::Returned(Value::Integer(0))
    );
    let update = runner.compile("pending[1]=25; return pending[1];")?;
    assert_eq!(
        runner.start(&update)?.resume(1000)?,
        Execution::Returned(Value::Integer(25))
    );
    Ok(())
}

#[test]
fn production_array_identity_is_shared_not_structural() -> Result {
    let program = compile("local a=[1]; local b=a; return a==b && a!=[1];")?;
    assert_eq!(
        Vm::new(&program)?.resume(1000)?,
        Execution::Returned(Value::Bool(true))
    );
    Ok(())
}
