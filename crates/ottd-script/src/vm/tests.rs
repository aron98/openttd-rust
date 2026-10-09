//! Private register-state witnesses captured from the actual bundled native VM.
use super::{Execution, Value, Vm};
use crate::compile;
use std::fmt::Write;
fn scalar(value: Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Integer(n) => format!("integer {n}"),
        Value::Float(bits) => format!("float {bits}"),
        Value::Bool(b) => format!("bool {}", u8::from(b)),
    }
}
fn check_frames(
    source: &str,
    native: &str,
    credits: &[u32],
) -> Result<(), Box<dyn std::error::Error>> {
    // Given: independently captured native state and source bytes.
    let program = compile(source)?;
    let mut vm = Vm::new(&program)?;
    let mut actual = String::new();
    // When: the real VM resumes on the same credit sequence as native.
    for &credit in credits {
        match vm.resume(credit)? {
            Execution::Suspended => {
                writeln!(
                    actual,
                    "suspend {} {}",
                    vm.remaining_ops(),
                    vm.instruction_pointer()
                )?;
                for (slot, value) in vm.registers.iter().enumerate().skip(1) {
                    writeln!(actual, "frame {slot} {}", scalar(*value))?;
                }
            }
            Execution::Returned(value) => {
                writeln!(actual, "return {} {}", vm.remaining_ops(), scalar(value))?;
                break;
            }
        }
    }
    // Then: every private scalar register, IP, debt and returned value agrees.
    let mut expected = String::new();
    for line in native.lines().filter(|line| {
        line.starts_with("frame ") || line.starts_with("suspend ") || line.starts_with("return ")
    }) {
        writeln!(expected, "{line}")?;
    }
    assert_eq!(actual, expected);
    Ok(())
}
#[test]
fn scope_clear_matches_native_frames() -> Result<(), Box<dyn std::error::Error>> {
    check_frames(
        include_str!("../../../../scripts/compat/script-vm/fixtures/branch_scope_guard_clear.nut"),
        include_str!("../../tests/frames/scope_guard_clear.txt"),
        &[3, 2, 2, 100],
    )
}
#[test]
fn scope_noop_matches_native_frames() -> Result<(), Box<dyn std::error::Error>> {
    check_frames(
        include_str!("../../../../scripts/compat/script-vm/fixtures/branch_scope_guard_noop.nut"),
        include_str!("../../tests/frames/scope_guard_noop.txt"),
        &[3, 2, 2, 100],
    )
}
#[test]
fn local_alias_matches_native_frames() -> Result<(), Box<dyn std::error::Error>> {
    check_frames(
        include_str!("../../../../scripts/compat/script-vm/fixtures/branch_local_alias.nut"),
        include_str!("../../tests/frames/local_alias.txt"),
        &[3, 2, 2, 100],
    )
}

#[test]
fn expression_assignment_matches_native_frames() -> Result<(), Box<dyn std::error::Error>> {
    check_frames(
        include_str!("../../../../scripts/compat/script-vm/fixtures/branch_expstate_target.nut"),
        include_str!("../../tests/frames/expstate_target.txt"),
        &[3, 2, 2, 2, 2, 100],
    )
}
