//! Forged string cursors are tested only in Rust, never executed by native.
use ottd_script::{Execution, Instruction, Program, Realm, Value, Vm, VmError};
use std::error::Error;
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const fn op(opcode: u8, arg0: u8, arg1: i32, arg2: u8, arg3: u8) -> Instruction {
    Instruction {
        opcode,
        arg0,
        arg1,
        arg2,
        arg3,
    }
}
fn program(cursor: Value, instruction: Instruction) -> Result<Program> {
    Ok(Program::from_parts(
        5,
        vec![Value::String(Realm::new().string(b"x")), cursor],
        vec![
            op(0x01, 1, 0, 0, 0),
            op(0x01, 4, 1, 0, 0),
            instruction,
            op(0x34, 255, i32::MAX, 255, 255),
            op(0x13, 1, 3, 0, 0),
        ],
    )?)
}
#[test]
fn malformed_string_cursor_is_rejected_before_indexing() -> Result {
    for cursor in [
        Value::Integer(-1),
        Value::Integer(i64::MIN),
        Value::Bool(false),
        Value::Float(0),
        Value::String(Realm::new().string(b"0")),
    ] {
        let program = program(cursor, op(0x33, 1, 1, 2, 0))?;
        assert_eq!(
            Vm::new(&program)?.resume(100),
            Err(VmError::InvalidBytecode)
        );
    }
    Ok(())
}
#[test]
fn exhausted_string_cursor_preserves_unwritten_value() -> Result {
    for cursor in [Value::Integer(1), Value::Integer(i64::MAX)] {
        let program = program(cursor, op(0x33, 1, 1, 2, 0))?;
        assert_eq!(
            Vm::new(&program)?.resume(100)?,
            Execution::Returned(Value::Null)
        );
    }
    Ok(())
}
#[test]
fn malformed_string_registers_and_reserved_operand_are_rejected() -> Result {
    for instruction in [
        op(0x33, 255, 1, 2, 0),
        op(0x33, 5, 1, 2, 0),
        op(0x33, 1, 1, 0, 0),
        op(0x33, 1, 1, 1, 0),
        op(0x33, 3, 1, 2, 0),
        op(0x33, 1, 1, 3, 0),
        op(0x33, 1, 1, 254, 0),
        op(0x33, 1, 1, 2, 1),
    ] {
        let program = program(Value::Null, instruction)?;
        assert_eq!(
            Vm::new(&program)?.resume(100),
            Err(VmError::InvalidBytecode)
        );
    }
    Ok(())
}
#[test]
fn string_checks_only_taken_branch_and_skips_invalid_post() -> Result {
    for offset in [i32::MIN, i32::MAX] {
        let populated = program(Value::Null, op(0x33, 1, offset, 2, 0))?;
        assert_eq!(
            Vm::new(&populated)?.resume(100)?,
            Execution::Returned(Value::Integer(120))
        );
        let exhausted = program(Value::Integer(1), op(0x33, 1, offset, 2, 0))?;
        assert_eq!(
            Vm::new(&exhausted)?.resume(100),
            Err(VmError::InvalidBytecode)
        );
    }
    let reached = program(Value::Integer(1), op(0x33, 1, 0, 2, 0))?;
    assert_eq!(
        Vm::new(&reached)?.resume(100),
        Err(VmError::UnsupportedOpcode(0x34))
    );
    Ok(())
}
#[test]
fn string_successful_skip_must_land_in_program() -> Result {
    let program = Program::from_parts(
        5,
        vec![Value::String(Realm::new().string(b"x"))],
        vec![op(0x01, 1, 0, 0, 0), op(0x33, 1, 0, 2, 0)],
    )?;
    assert_eq!(
        Vm::new(&program)?.resume(100),
        Err(VmError::InvalidBytecode)
    );
    Ok(())
}
