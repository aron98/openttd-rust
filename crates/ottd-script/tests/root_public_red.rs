//! Public production regressions, captured failing before the root implementation.
use ottd_script::{Execution, Realm, Value, Vm};
#[test]
fn unresolved_identifier_is_runtime_lookup() {
    let realm = Realm::new();
    let program = realm.compile("return score;");
    assert!(
        program.is_ok(),
        "native compiles unresolved root reads: {program:?}"
    );
}
#[test]
fn root_creation_and_assignment_execute() -> Result<(), Box<dyn std::error::Error>> {
    let program = Realm::new().compile("score <- 1; score = 5; return score;")?;
    let mut vm = Vm::new(&program)?;
    assert_eq!(vm.resume(1000), Ok(Execution::Returned(Value::Integer(5))));
    Ok(())
}
