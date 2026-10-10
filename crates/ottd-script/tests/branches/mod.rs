//! Branch safety and compatibility boundaries outside the native source matrix.
use ottd_script::{Execution, Instruction, Program, Value, Vm, VmError, compile};

#[test]
fn taken_jump_rejects_invalid_signed_targets() -> Result<(), Box<dyn std::error::Error>> {
    for offset in [i32::MIN, i32::MAX] {
        // Given: a public program with an unrepresentable/out-of-frame jump.
        let program = Program::from_parts(
            1,
            vec![],
            vec![Instruction {
                opcode: 0x18,
                arg0: 0,
                arg1: offset,
                arg2: 0,
                arg3: 0,
            }],
        )?;
        // When: the branch executes.
        let result = Vm::new(&program)?.resume(2);
        // Then: invalid targets do not wrap or escape checked bytecode.
        assert_eq!(result, Err(VmError::InvalidBytecode));
    }
    Ok(())
}

#[test]
fn comparison_rejects_invalid_selector() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a syntactically representable comparison with an unknown selector.
    let program = Program::from_parts(
        2,
        vec![],
        vec![Instruction {
            opcode: 0x28,
            arg0: 1,
            arg1: 1,
            arg2: 1,
            arg3: 255,
        }],
    )?;
    // When: dispatching it.
    let result = Vm::new(&program)?.resume(2);
    // Then: it cannot silently choose another comparison.
    assert_eq!(result, Err(VmError::InvalidBytecode));
    Ok(())
}

#[test]
fn literal_comparison_rejects_missing_pool_entry() -> Result<(), Box<dyn std::error::Error>> {
    // Given: EQ's native literal flag with an empty constant pool.
    let program = Program::from_parts(
        2,
        vec![],
        vec![Instruction {
            opcode: 0x0f,
            arg0: 1,
            arg1: 0,
            arg2: 1,
            arg3: 255,
        }],
    )?;
    // When: resolving the operand.
    let result = Vm::new(&program)?.resume(2);
    // Then: constant indices are checked independently of register indices.
    assert_eq!(result, Err(VmError::InvalidBytecode));
    Ok(())
}

#[test]
fn scope_cleanup_rejects_reserved_and_absent_start() -> Result<(), Box<dyn std::error::Error>> {
    for start in [0, 3] {
        // Given: cleanup beginning at the root slot or outside a two-slot frame.
        let program = Program::from_parts(
            2,
            vec![],
            vec![Instruction {
                opcode: 0x3d,
                arg0: start,
                arg1: i32::from(start),
                arg2: 0,
                arg3: 0,
            }],
        )?;
        // When: cleanup dispatches.
        let result = Vm::new(&program)?.resume(2);
        // Then: the invalid range is refused before mutation.
        assert_eq!(result, Err(VmError::InvalidBytecode));
    }
    Ok(())
}

#[test]
fn scope_cleanup_rejects_counter_overflow() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the signed native cleanup count cannot be represented.
    let program = Program::from_parts(
        2,
        vec![],
        vec![Instruction {
            opcode: 0x3d,
            arg0: 1,
            arg1: i32::MAX,
            arg2: 0,
            arg3: 0,
        }],
    )?;
    // When: computing the count.
    let result = Vm::new(&program)?.resume(2);
    // Then: checked arithmetic reports the corrupt program.
    assert_eq!(result, Err(VmError::InvalidBytecode));
    Ok(())
}

#[test]
fn unsupported_language_is_rejected_even_when_unreachable() {
    for source in [
        "if(false){return missing;} return 1;",
        "local x=1; x<-2;",
        "foreach(x in [1]){}",
        "try{}catch(e){}",
    ] {
        // Given: source explicitly outside the approved local/branch subset.
        // When: compiling, including unreachable branches.
        let result = compile(source);
        // Then: no host or object semantics are invented.
        assert!(result.is_err(), "unexpected support for {source}");
    }
}

#[test]
fn native_negative_cleanup_count_preserves_valid_nested_loop()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: native last_stacksize survives shrinking inner scopes.
    let program = compile(include_str!(
        "../../../../scripts/compat/script-vm/fixtures/branch_nested_cleanup.nut"
    ))?;
    // When: the outer break's signed cleanup count is negative.
    let result = Vm::new(&program)?.resume(100)?;
    // Then: it performs no stores and returns normally, as native does.
    assert_eq!(result, Execution::Returned(Value::Integer(5)));
    Ok(())
}

#[test]
fn finite_credits_suspend_infinite_loop() -> Result<(), Box<dyn std::error::Error>> {
    // Given: an intentional infinite loop.
    let program = compile("while(true) {}")?;
    let mut vm = Vm::new(&program)?;
    // When: repeatedly funding a bounded amount of work.
    let observations = [0, 1, 2, 3, 5, 10, 100]
        .into_iter()
        .map(|credit| vm.resume(credit))
        .collect::<Result<Vec<_>, _>>()?;
    // Then: each resume suspends, with no fabricated return or unbounded run.
    assert_eq!(observations, vec![Execution::Suspended; 7]);
    assert_eq!((vm.remaining_ops(), vm.instruction_pointer()), (0, 2));
    Ok(())
}
