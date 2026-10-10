//! Private register-state witnesses captured from the actual bundled native VM.
use super::{Execution, Value, Vm};
use crate::compile;
use std::fmt::Write;
fn scalar(value: &Value) -> String {
    match value {
        Value::String(bytes) => format!(
            "string {} {}",
            bytes.as_bytes().len(),
            bytes
                .as_bytes()
                .iter()
                .fold(String::new(), |mut output, byte| {
                    write!(output, "{byte:02x}").expect("String formatting is infallible");
                    output
                })
        ),
        Value::Null => "null".to_owned(),
        Value::Integer(n) => format!("integer {n}"),
        Value::Float(bits) => format!("float {bits}"),
        Value::Bool(b) => format!("bool {}", u8::from(*b)),
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
        match vm.resume(credit) {
            Ok(Execution::Suspended) => {
                writeln!(
                    actual,
                    "suspend {} {}",
                    vm.remaining_ops(),
                    vm.instruction_pointer()
                )?;
                for (slot, value) in vm.registers.iter().enumerate().skip(1) {
                    writeln!(actual, "frame {slot} {}", scalar(value))?;
                }
            }
            Ok(Execution::Returned(value)) => {
                writeln!(actual, "return {} {}", vm.remaining_ops(), scalar(&value))?;
                break;
            }
            Err(_) => {
                writeln!(actual, "runtime_error {}", vm.remaining_ops())?;
                break;
            }
        }
    }
    // Then: every private scalar register, IP, debt and returned value agrees.
    let mut expected = String::new();
    for line in native.lines().filter(|line| {
        line.starts_with("frame ")
            || line.starts_with("suspend ")
            || line.starts_with("return ")
            || line.starts_with("runtime_error ")
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

#[test]
fn iteration_update_frame_matches_native_frames() -> Result<(), Box<dyn std::error::Error>> {
    check_frames(
        include_str!(
            "../../../../scripts/compat/script-vm/fixtures/branch_iteration_update_frame.nut"
        ),
        include_str!("../../tests/frames/iteration_update_frame.txt"),
        &[
            2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 100,
        ],
    )
}

#[test]
fn iteration_loop_frame_matches_native_frames() -> Result<(), Box<dyn std::error::Error>> {
    check_frames(
        include_str!(
            "../../../../scripts/compat/script-vm/fixtures/branch_iteration_loop_frame.nut"
        ),
        include_str!("../../tests/frames/iteration_loop_frame.txt"),
        &[
            2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 100,
        ],
    )
}

#[test]
fn failed_update_matches_native_frames() -> Result<(), Box<dyn std::error::Error>> {
    let source = include_str!(
        "../../../../scripts/compat/script-vm/fixtures/branch_iteration_failed_update.nut"
    );
    check_frames(
        source,
        include_str!("../../tests/frames/iteration_failed_update.txt"),
        &[2, 2, 100],
    )?;
    let program = compile(source)?;
    let mut vm = Vm::new(&program)?;
    // Observe the failed store before native-style terminal frame unwinding.
    let update_position = program
        .instructions()
        .iter()
        .position(|instruction| instruction.opcode == 0x23)
        .ok_or("missing failed update instruction")?;
    for instruction in program.instructions().iter().take(update_position + 1) {
        if instruction.opcode == 0x23 {
            assert_eq!(vm.update(*instruction), Err(crate::VmError::DivisionByZero));
            assert_eq!(vm.registers.get(1), Some(&Value::Integer(7)));
            break;
        }
        let _result = vm.step(*instruction)?;
    }
    let mut vm = Vm::new(&program)?;
    assert_eq!(vm.resume(100), Err(crate::VmError::DivisionByZero));
    assert!(vm.registers.iter().all(|value| *value == Value::Null));
    Ok(())
}

#[test]
fn switch_case_frame_matches_native_frames() -> Result<(), Box<dyn std::error::Error>> {
    check_frames(
        include_str!("../../../../scripts/compat/script-vm/fixtures/branch_switch_case_frame.nut"),
        include_str!("../../tests/frames/switch_case_frame.txt"),
        &[
            2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 100,
        ],
    )
}

#[test]
fn switch_continue_frame_matches_native_frames() -> Result<(), Box<dyn std::error::Error>> {
    check_frames(
        include_str!(
            "../../../../scripts/compat/script-vm/fixtures/branch_switch_continue_frame.nut"
        ),
        include_str!("../../tests/frames/switch_continue_frame.txt"),
        &[
            2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 100,
        ],
    )
}

#[test]
fn strings_load_frame_matches_native_frames() -> Result<(), Box<dyn std::error::Error>> {
    check_frames(
        include_str!("../../../../scripts/compat/script-vm/fixtures/strings_load_frame.nut"),
        include_str!("../../tests/frames/strings_load_frame.txt"),
        &[
            2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 100,
        ],
    )
}

#[test]
fn strings_update_frame_matches_native_frames() -> Result<(), Box<dyn std::error::Error>> {
    check_frames(
        include_str!("../../../../scripts/compat/script-vm/fixtures/strings_update_frame.nut"),
        include_str!("../../tests/frames/strings_update_frame.txt"),
        &[
            2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 100,
        ],
    )
}

#[test]
fn strings_failure_frame_matches_native_frames() -> Result<(), Box<dyn std::error::Error>> {
    check_frames(
        include_str!("../../../../scripts/compat/script-vm/fixtures/strings_failure_frame.nut"),
        include_str!("../../tests/frames/strings_failure_frame.txt"),
        &[
            2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 100,
        ],
    )
}
