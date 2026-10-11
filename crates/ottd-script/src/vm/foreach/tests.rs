//! Direct dispatch preserves owning slots on exhaustion and validation failure.
use super::*;
use crate::{Program, Realm, Temporary};

#[test]
fn string_exhaustion_and_errors_preserve_frame_and_temporary() -> Result<(), VmError> {
    let realm = Realm::new();
    for (cursor, offset, expected) in [
        (1, 0, Ok(())),
        (-1, 0, Err(VmError::InvalidBytecode)),
        (1, i32::MAX, Err(VmError::InvalidBytecode)),
    ] {
        let instruction = Instruction {
            opcode: 0x33,
            arg0: 1,
            arg1: offset,
            arg2: 2,
            arg3: 0,
        };
        let program = Program::from_parts(5, vec![], vec![instruction])?;
        let mut vm = Vm::new(&program)?;
        vm.write(1, Value::String(realm.string(b"x")))?;
        vm.write(2, Value::Integer(77))?;
        vm.write(3, Value::String(realm.string(b"retained")))?;
        vm.write(4, Value::Integer(cursor))?;
        vm.runner.get_mut().temporary = Temporary::Value(Value::String(realm.string(b"temporary")));
        assert_eq!(vm.foreach(instruction), expected);
        assert_eq!(vm.register(2)?, Value::Integer(77));
        assert_eq!(vm.register(3)?, Value::String(realm.string(b"retained")));
        assert_eq!(vm.register(4)?, Value::Integer(cursor));
        assert!(
            matches!(&vm.runner.get().temporary, Temporary::Value(Value::String(s)) if s.as_bytes()==b"temporary")
        );
        assert!(matches!(vm.registers.first(), Some(Value::Table(_))));
    }
    Ok(())
}
