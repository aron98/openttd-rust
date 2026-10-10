//! Public bytecode validation around the array-only foreach dispatch.
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
fn run(
    cursor: Value,
    instruction: Instruction,
    populated: bool,
) -> Result<std::result::Result<Execution, VmError>> {
    let mut code = vec![op(0x1f, 1, 0, 0, 0), op(0x02, 5, 7, 0, 0)];
    if populated {
        code.push(op(0x20, 1, 5, 0, 0));
    }
    code.extend([
        op(0x01, 4, 0, 0, 0),
        instruction,
        op(0x34, 1, 1, 2, 0),
        op(0x13, 1, 3, 0, 0),
    ]);
    let program = Program::from_parts(6, vec![cursor], code)?;
    Ok(Vm::new(&program)?.resume(100))
}
#[test]
fn valid_cursor_success_and_exhaustion() -> Result {
    let instruction = op(0x33, 1, 1, 2, 0);
    assert_eq!(
        run(Value::Null, instruction, true)?,
        Ok(Execution::Returned(Value::Integer(7)))
    );
    for cursor in [
        Value::Integer(-1),
        Value::Integer(i64::MIN),
        Value::Integer(1),
        Value::Integer(i64::MAX),
    ] {
        assert_eq!(
            run(cursor, instruction, true)?,
            Ok(Execution::Returned(Value::Null))
        );
    }
    assert_eq!(
        run(Value::Null, instruction, false)?,
        Ok(Execution::Returned(Value::Null))
    );
    Ok(())
}
#[test]
fn malformed_registers_reserved_operand_and_overlap_fail() -> Result {
    for instruction in [
        op(0x33, 0, 1, 2, 0),
        op(0x33, 6, 1, 2, 0),
        op(0x33, 1, 1, 0, 0),
        op(0x33, 1, 1, 1, 0),
        op(0x33, 3, 1, 2, 0),
        op(0x33, 1, 1, 4, 0),
        op(0x33, 1, 1, 254, 0),
        op(0x33, 1, 1, 2, 1),
    ] {
        assert_eq!(
            run(Value::Null, instruction, true)?,
            Err(VmError::InvalidBytecode)
        );
    }
    Ok(())
}
#[test]
fn only_taken_branch_is_bounds_checked() -> Result {
    for offset in [i32::MIN, i32::MAX] {
        let instruction = op(0x33, 1, offset, 2, 0);
        assert_eq!(
            run(Value::Null, instruction, true)?,
            Ok(Execution::Returned(Value::Integer(7)))
        );
        assert_eq!(
            run(Value::Null, instruction, false)?,
            Err(VmError::InvalidBytecode)
        );
    }
    let program = Program::from_parts(5, vec![], vec![op(0x1f, 1, 0, 0, 0), op(0x33, 1, 0, 2, 0)])?;
    assert_eq!(
        Vm::new(&program)?.resume(100),
        Err(VmError::InvalidBytecode)
    );
    Ok(())
}
#[test]
fn cursor_type_is_bytecode_error_not_a_new_native_policy() -> Result {
    let realm = Realm::new();
    for cursor in [
        Value::Bool(false),
        Value::Float(0),
        Value::String(realm.string(b"0")),
    ] {
        assert_eq!(
            run(cursor, op(0x33, 1, 1, 2, 0), true)?,
            Err(VmError::InvalidBytecode)
        );
    }
    let program = Program::from_parts(
        5,
        vec![],
        vec![
            op(0x1f, 1, 0, 0, 0),
            op(0x0a, 4, 1, 0, 0),
            op(0x33, 1, 1, 2, 0),
            op(0x34, 1, 1, 2, 0),
            op(0x13, 255, 0, 0, 0),
        ],
    )?;
    assert_eq!(
        Vm::new(&program)?.resume(100),
        Err(VmError::InvalidBytecode)
    );
    Ok(())
}
#[test]
fn unsupported_iterable_and_non_iterable_have_distinct_boundaries() -> Result {
    for (source, expected) in [
        ("foreach(v in \"abc\") ;", VmError::UnsupportedReceiver),
        ("foreach(v in null) ;", VmError::OperandType),
        ("foreach(v in 7) ;", VmError::OperandType),
        ("foreach(v in true) ;", VmError::OperandType),
        ("foreach(v in 1.0) ;", VmError::OperandType),
    ] {
        let program = ottd_script::compile(source)?;
        assert_eq!(Vm::new(&program)?.resume(100), Err(expected));
    }
    let program = Program::from_parts(1, vec![], vec![op(0x34, 0, 0, 0, 0)])?;
    assert_eq!(
        Vm::new(&program)?.resume(100),
        Err(VmError::UnsupportedOpcode(0x34))
    );
    Ok(())
}

#[test]
fn successful_skip_is_checked_and_receiver_precedes_cursor_type() -> Result {
    let program = Program::from_parts(
        6,
        vec![],
        vec![
            op(0x1f, 1, 0, 0, 0),
            op(0x02, 5, 7, 0, 0),
            op(0x20, 1, 5, 0, 0),
            op(0x33, 1, 0, 2, 0),
        ],
    )?;
    assert_eq!(
        Vm::new(&program)?.resume(100),
        Err(VmError::InvalidBytecode)
    );
    let realm = Realm::new();
    for (receiver, error) in [
        (
            Value::String(realm.string(b"x")),
            VmError::UnsupportedReceiver,
        ),
        (Value::Null, VmError::OperandType),
    ] {
        let program = Program::from_parts(
            5,
            vec![receiver, Value::Bool(false)],
            vec![
                op(0x01, 1, 0, 0, 0),
                op(0x01, 4, 1, 0, 0),
                op(0x33, 1, 1, 2, 0),
                op(0x34, 1, 1, 2, 0),
                op(0x13, 255, 0, 0, 0),
            ],
        )?;
        assert_eq!(Vm::new(&program)?.resume(100), Err(error));
    }
    Ok(())
}

#[path = "foreach_public.rs"]
mod production_regressions;
