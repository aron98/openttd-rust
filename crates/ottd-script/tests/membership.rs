//! Exact source bodies with original-success RED receipts in the approved delegate design.
use ottd_script::{Execution, Value, Vm, compile};

fn execute(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(source)?;
    for credits in [
        vec![10000],
        vec![0, 1, 2, 4, 0, 8, 16, 32, 10000],
        [vec![2; 40], vec![10000]].concat(),
    ] {
        let mut vm = Vm::new(&program)?;
        let mut result = Ok(Execution::Suspended);
        for credit in credits {
            result = vm.resume(credit);
            if result != Ok(Execution::Suspended) {
                break;
            }
        }
        assert_eq!(result, Ok(Execution::Returned(Value::Integer(1))));
    }
    Ok(())
}

#[test]
fn array_numeric() {
    execute(include_str!("membership/array-numeric.nut")).expect("defined native membership");
}

#[test]
fn bitwise_precedence() {
    execute(include_str!("membership/bitwise-precedence.nut")).expect("defined native membership");
}

#[test]
fn left_associative_chain() {
    execute(include_str!("membership/left-associative-chain.nut"))
        .expect("defined native membership");
}

#[test]
fn no_default_method() {
    execute(include_str!("membership/no-default-method.nut")).expect("defined native membership");
}

#[test]
fn own_and_missing() {
    execute(include_str!("membership/own-and-missing.nut")).expect("defined native membership");
}

#[test]
fn own_null_present() {
    execute(include_str!("membership/own-null-present.nut")).expect("defined native membership");
}

#[test]
fn scalar_missing() {
    execute(include_str!("membership/scalar-missing.nut")).expect("defined native membership");
}

#[test]
fn short_circuit() {
    execute(include_str!("membership/short-circuit.nut")).expect("defined native membership");
}

#[test]
fn string_negative() {
    execute(include_str!("membership/string-negative.nut")).expect("defined native membership");
}
