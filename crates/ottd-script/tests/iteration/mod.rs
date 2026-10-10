//! Native scalar update and iteration behavior.
use ottd_script::{Execution, Value, Vm, compile};
#[test]
fn native_scalar_iteration_examples() -> Result<(), Box<dyn std::error::Error>> {
    for (source, expected) in [
        ("return -1>>>1;", i64::MAX),
        ("return 1<<63;", i64::MIN),
        ("return ++2;", 3),
        ("local a=1; return a+=(a=2);", 4),
        ("local a=1; return a+++a;", 3),
        ("return false?1/0:7;", 7),
        ("local a=1; return a=2,a+1;", 3),
        ("local s=0; for(local i=0;i<4;i++){s+=i;} return s;", 6),
        ("local a=0; do {a++;} while(false); return a;", 1),
    ] {
        let program = compile(source)?;
        assert_eq!(
            Vm::new(&program)?.resume(1000)?,
            Execution::Returned(Value::Integer(expected)),
            "{source}"
        );
    }
    Ok(())
}

#[test]
fn cpp20_defined_shifts_preserve_all_i64_patterns() -> Result<(), Box<dyn std::error::Error>> {
    for (source, expected) in [
        ("return -1<<63;", i64::MIN),
        ("return 0x8000000000000000<<1;", 0),
        ("return -3>>1;", -2),
        ("return -1>>>63;", 1),
    ] {
        let program = compile(source)?;
        assert_eq!(
            Vm::new(&program)?.resume(100)?,
            Execution::Returned(Value::Integer(expected))
        );
    }
    Ok(())
}

#[test]
fn undefined_shift_counts_are_distinct_from_native_type_errors()
-> Result<(), Box<dyn std::error::Error>> {
    for operator in ["<<", ">>", ">>>"] {
        for count in [-1, 64, i64::MAX] {
            let program = compile(&format!("return 1{operator}{count};"))?;
            assert_eq!(
                Vm::new(&program)?.resume(100),
                Err(ottd_script::VmError::UnsupportedShiftCount)
            );
        }
    }
    let program = compile("return 1<<true;")?;
    assert_eq!(
        Vm::new(&program)?.resume(100),
        Err(ottd_script::VmError::OperandType)
    );
    Ok(())
}

#[test]
fn update_overflow_remains_explicit() -> Result<(), Box<dyn std::error::Error>> {
    for operation in ["++a", "a++", "a+=1"] {
        let program = compile(&format!("local a=0x7fffffffffffffff;return {operation};"))?;
        assert_eq!(
            Vm::new(&program)?.resume(100),
            Err(ottd_script::VmError::UnsupportedOverflow)
        );
    }
    Ok(())
}

#[test]
fn bitwise_selector_hole_is_invalid_bytecode() -> Result<(), Box<dyn std::error::Error>> {
    let mut program = compile("return 1&2;")?;
    for instruction in &mut program.instructions {
        if instruction.opcode == 0x12 {
            instruction.arg3 = 1;
        }
    }
    assert_eq!(
        Vm::new(&program)?.resume(100),
        Err(ottd_script::VmError::InvalidBytecode)
    );
    Ok(())
}

#[test]
fn native_for_increment_reemission_can_preserve_nontermination()
-> Result<(), Box<dyn std::error::Error>> {
    let program = compile("local a=0; for(;a<3;a=a<1?1:a+1){} return a;")?;
    let mut vm = Vm::new(&program)?;
    assert_eq!(vm.resume(10000)?, Execution::Suspended);
    assert_eq!((vm.remaining_ops(), vm.instruction_pointer()), (0, 13));
    Ok(())
}

#[test]
fn aliased_postfix_destination_observes_native_store_order()
-> Result<(), Box<dyn std::error::Error>> {
    let mut program = compile("local a=2; return a;")?;
    program.instructions.insert(
        1,
        ottd_script::Instruction {
            opcode: 0x27,
            arg0: 1,
            arg1: 1,
            arg2: 0,
            arg3: 255,
        },
    );
    assert_eq!(
        Vm::new(&program)?.resume(100)?,
        Execution::Returned(Value::Integer(1))
    );
    Ok(())
}
