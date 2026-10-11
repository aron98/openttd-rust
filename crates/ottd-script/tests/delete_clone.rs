//! Language clone identity and checked malformed opcode boundaries.
use ottd_script::{Execution, Instruction, Program, Value, Vm, VmError, compile};
#[test]
fn clone_survives_source_and_execution_owners() {
    let copy = {
        let program = compile("local t={key=7};\nreturn clone t;").expect("source");
        let Execution::Returned(Value::Table(copy)) =
            Vm::new(&program).expect("vm").resume(100).expect("result")
        else {
            panic!("table")
        };
        copy
    };
    let realm = ottd_script::Realm::new();
    assert_eq!(
        copy.raw_get(Value::String(realm.string(b"key"))),
        Ok(Some(Value::Integer(7)))
    );
}
#[test]
fn malformed_delete_clone_fields_are_checked() {
    for (opcode, arg0, arg1, arg2, arg3) in [
        (0x36, 1, -1, 0, 0),
        (0x36, 1, 2, 0, 0),
        (0x36, 1, 1, 1, 0),
        (0x36, 1, 1, 0, 1),
        (0x0c, 1, -1, 1, 0),
        (0x0c, 1, 2, 1, 0),
        (0x0c, 1, 1, 2, 0),
        (0x0c, 1, 1, 1, 1),
    ] {
        let program = Program::from_parts(
            2,
            vec![],
            vec![Instruction {
                opcode,
                arg0,
                arg1,
                arg2,
                arg3,
            }],
        )
        .expect("parts");
        assert_eq!(
            Vm::new(&program).expect("vm").resume(10),
            Err(VmError::InvalidBytecode)
        );
    }
}

#[test]
fn clone_rejects_reserved_and_out_of_frame_destinations() {
    for arg0 in [0, 255] {
        let program = Program::from_parts(
            2,
            vec![],
            vec![
                Instruction {
                    opcode: 0x1f,
                    arg0: 1,
                    arg1: 0,
                    arg2: 0,
                    arg3: 0,
                },
                Instruction {
                    opcode: 0x36,
                    arg0,
                    arg1: 1,
                    arg2: 0,
                    arg3: 0,
                },
            ],
        )
        .expect("parts");
        assert_eq!(
            Vm::new(&program).expect("vm").resume(10),
            Err(VmError::InvalidBytecode)
        );
    }
}
