//! Native expression-state boundaries are independent of assignment destinations.
use ottd_script::{Execution, Value, Vm, compile};

#[test]
fn parentheses_do_not_export_assignment_validity() {
    for expression in ["(a)=2", "((a))=2", "(a=2)=3", "a+(a)=2", "-(a)=2"] {
        assert!(
            compile(&format!("local a=1; return {expression};")).is_err(),
            "native rejects {expression}"
        );
    }
}

#[test]
fn final_local_factor_allows_assignment_to_expression_target()
-> Result<(), Box<dyn std::error::Error>> {
    for expression in ["a+a=2", "-a=2", "!a=2", "~a=2", "a||a=2", "a&&a=2", "2<a=2"] {
        let program = compile(&format!("local a=1; return {expression};"))?;
        assert_eq!(
            Vm::new(&program)?.resume(100)?,
            Execution::Returned(Value::Integer(2))
        );
    }
    Ok(())
}

#[test]
fn assignment_writes_target_without_rebinding_final_factor()
-> Result<(), Box<dyn std::error::Error>> {
    let program = compile(include_str!(
        "../../../../scripts/compat/script-vm/fixtures/branch_expstate_target.nut"
    ))?;
    assert_eq!(
        Vm::new(&program)?.resume(100)?,
        Execution::Returned(Value::Integer(127))
    );
    Ok(())
}

#[test]
fn assignment_rhs_restores_outer_expression_state() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile("local a=1,b=2; a=(b=3); return a*10+b;")?;
    assert_eq!(
        Vm::new(&program)?.resume(100)?,
        Execution::Returned(Value::Integer(33))
    );
    assert!(compile("local a=1,b=2; return a+(b=3)=4;").is_err());
    Ok(())
}
