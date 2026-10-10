//! Independent scalar compilation/execution regression checks.
use ottd_script::{Execution, Value, Vm, compile};
#[test]
fn arithmetic_precedence_when_returning_expression() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a real program with distinguishable precedence.
    let source = "return 7 + 3 * (8 - 2);";
    // When: independently compiling and executing it.
    let program = compile(source)?;
    let result = Vm::new(&program)?.resume(100)?;
    // Then: multiplication occurs before addition.
    assert_eq!(result, Execution::Returned(Value::Integer(25)));
    Ok(())
}

#[test]
fn debt_and_ip_persist_when_resuming() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the native probe's known instruction sequence and budget credits.
    let program = compile("return 7 + 3 * (8 - 2);")?;
    let mut vm = Vm::new(&program)?;
    // When: credits are applied to the same suspended frame.
    let mut trace = Vec::new();
    for credit in [0, 1, 2, 3, 100] {
        trace.push((
            vm.resume(credit)?,
            vm.remaining_ops(),
            vm.instruction_pointer(),
        ));
    }
    // Then: each suspension charges an operation without advancing the IP.
    assert_eq!(
        trace,
        vec![
            (Execution::Suspended, -1, 0),
            (Execution::Suspended, -1, 0),
            (Execution::Suspended, 0, 0),
            (Execution::Suspended, 0, 2),
            (Execution::Returned(Value::Integer(25)), 94, 8),
        ]
    );
    Ok(())
}

#[test]
fn negative_zero_bits_survive_when_negating_float() -> Result<(), Box<dyn std::error::Error>> {
    // Given: Squirrel unary float negation.
    let program = compile("return -0.0;")?;
    // When: the compiled NEG instruction executes.
    let result = Vm::new(&program)?.resume(100)?;
    // Then: the sign bit survives.
    assert_eq!(result, Execution::Returned(Value::Float(0x8000_0000)));
    Ok(())
}

#[test]
fn signed_overflow_is_explicit_when_native_domain_is_undefined()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: signed native overflow, intentionally outside this increment's domain.
    let program = compile("return 9223372036854775807 + 1;")?;
    // When: execution reaches addition.
    let result = Vm::new(&program)?.resume(100);
    // Then: it cannot silently wrap or saturate.
    assert_eq!(result, Err(ottd_script::VmError::UnsupportedOverflow));
    Ok(())
}

#[test]
fn minimum_division_is_explicit_when_native_domain_is_undefined()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: the other undefined signed arithmetic boundary.
    let program = compile("return 0x8000000000000000 / -1;")?;
    // When: division executes.
    let result = Vm::new(&program)?.resume(100);
    // Then: no hardware-dependent value is invented.
    assert_eq!(result, Err(ottd_script::VmError::UnsupportedOverflow));
    Ok(())
}

#[test]
fn host_construct_is_rejected_when_compiling_outside_supported_grammar() {
    // Given: a function requiring unimplemented host/package semantics.
    let source = "return AIController.GetTick();";
    // When: the scalar compiler sees it.
    let result = compile(source);
    // Then: it explicitly rejects, rather than evaluating a placeholder.
    assert!(matches!(
        result,
        Err(ottd_script::CompileError {
            kind: ottd_script::CompileErrorKind::UnsupportedSyntax,
            ..
        })
    ));
}

#[test]
fn nesting_limit_is_checked_when_source_is_deep() {
    // Given: recursively nested source at a hostile-input boundary.
    let source = format!("return {}1{};", "(".repeat(200), ")".repeat(200));
    // When: compiling it.
    let result = compile(&source);
    // Then: it returns a typed limit, without stack overflow.
    assert!(matches!(
        result,
        Err(ottd_script::CompileError {
            kind: ottd_script::CompileErrorKind::Limit,
            ..
        })
    ));
}

#[test]
fn invalid_register_is_checked_when_loading_bytecode() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a public program with a corrupt destination register.
    let program = ottd_script::Program {
        stack_size: 1,
        literals: vec![],
        instructions: vec![ottd_script::Instruction {
            opcode: 2,
            arg0: 9,
            arg1: 7,
            arg2: 0,
            arg3: 0,
        }],
    };
    // When: executing it.
    let result = Vm::new(&program)?.resume(10);
    // Then: checked indexing rejects the program.
    assert_eq!(result, Err(ottd_script::VmError::InvalidBytecode));
    Ok(())
}

#[test]
fn unsupported_opcode_is_explicit_when_executing_bytecode() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: CLASS, whose semantics are not implemented yet.
    let program = ottd_script::Program {
        stack_size: 1,
        literals: vec![],
        instructions: vec![ottd_script::Instruction {
            opcode: 0x3b,
            arg0: 0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        }],
    };
    // When: dispatching it.
    let result = Vm::new(&program)?.resume(10);
    // Then: no fake class value is produced.
    assert_eq!(result, Err(ottd_script::VmError::UnsupportedOpcode(0x3b)));
    Ok(())
}

#[test]
fn terminal_frame_is_rejected_when_resumed_again() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a completed main frame.
    let program = compile("return 7;")?;
    let mut vm = Vm::new(&program)?;
    assert_eq!(vm.resume(100)?, Execution::Returned(Value::Integer(7)));
    // When: a caller resumes it again.
    let result = vm.resume(10);
    // Then: the implicit trailing return cannot override the result.
    assert_eq!(result, Err(ottd_script::VmError::Finished));
    Ok(())
}

#[test]
fn root_environment_read_is_rejected_when_scalar_bytecode_mentions_it()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: register zero is the unimplemented root environment, not null.
    let program = ottd_script::Program {
        stack_size: 1,
        literals: vec![],
        instructions: vec![ottd_script::Instruction {
            opcode: 0x13,
            arg0: 1,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        }],
    };
    // When: hostile bytecode tries to return the environment as a scalar.
    let result = Vm::new(&program)?.resume(10);
    // Then: the VM cannot invent a null root table.
    assert_eq!(result, Err(ottd_script::VmError::InvalidBytecode));
    Ok(())
}

#[test]
fn adjacent_null_loads_preserve_native_budget_when_operands_fail()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: two null expressions whose adjacent loads native merges.
    let program = compile("return null + null;")?;
    let mut vm = Vm::new(&program)?;
    // When: the merged load executes before invalid arithmetic.
    let result = vm.resume(10_000);
    // Then: exactly the load and arithmetic dispatch are charged.
    assert_eq!(
        (result, vm.remaining_ops()),
        (Err(ottd_script::VmError::OperandType), 9998)
    );
    Ok(())
}

#[test]
fn form_feed_is_rejected_when_between_tokens() {
    // Given: a native-invalid form feed at the whitespace boundary.
    let source = "return\u{000c}1;";
    // When: lexing the byte after the return keyword.
    let result = compile(source);
    // Then: the exact original byte offset is reported instead of returning one.
    assert_eq!(
        result,
        Err(ottd_script::CompileError {
            offset: 6,
            kind: ottd_script::CompileErrorKind::UnsupportedSyntax,
        })
    );
}

#[test]
fn vertical_tab_is_rejected_when_between_tokens() {
    // Given: the adjacent control character that native already rejects.
    let source = "return\u{000b}1;";
    // When: lexing the byte after the return keyword.
    let result = compile(source);
    // Then: rejection retains its exact byte offset.
    assert_eq!(
        result,
        Err(ottd_script::CompileError {
            offset: 6,
            kind: ottd_script::CompileErrorKind::UnsupportedSyntax,
        })
    );
}

#[test]
fn horizontal_whitespace_is_accepted_without_ending_return()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: native space, tab and CR separators, with no LF before the value.
    let program = compile("return \t\r1;")?;
    // When: executing the resulting return expression.
    let result = Vm::new(&program)?.resume(100)?;
    // Then: CR is whitespace, not a statement-ending newline.
    assert_eq!(result, Execution::Returned(Value::Integer(1)));
    Ok(())
}

#[test]
fn line_feed_ends_return_when_preceded_by_carriage_return() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: a CRLF immediately after return.
    let program = compile("return\r\n;")?;
    // When: executing the statement ended by LF.
    let result = Vm::new(&program)?.resume(100)?;
    // Then: native's empty return semantics are preserved.
    assert_eq!(result, Execution::Returned(Value::Null));
    Ok(())
}

#[test]
fn assignment_preserves_native_aliasing_when_rhs_changes_local()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: the native left operand remains a reference to local a.
    let program = compile("local a=1,b=2; return a+(a=b);")?;
    // When: assignment changes a before arithmetic reads it.
    let result = Vm::new(&program)?.resume(100)?;
    // Then: both reads observe two, matching the native probe.
    assert_eq!(result, Execution::Returned(Value::Integer(4)));
    Ok(())
}

#[test]
fn loop_resumes_when_credit_runs_out_between_branches() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the pinned native while-sum probe.
    let program = compile("local n=0,total=0; while(n<4) { total=total+n; n=n+1; } return total;")?;
    let mut vm = Vm::new(&program)?;
    // When: separate credits fund one persistent frame.
    let mut trace = Vec::new();
    for credit in [0, 1, 2, 3, 5, 10, 100] {
        trace.push((
            vm.resume(credit)?,
            vm.remaining_ops(),
            vm.instruction_pointer(),
        ));
    }
    // Then: native suspension positions and operation debt are preserved.
    assert_eq!(
        trace,
        vec![
            (Execution::Suspended, -1, 0),
            (Execution::Suspended, -1, 0),
            (Execution::Suspended, 0, 0),
            (Execution::Suspended, 0, 2),
            (Execution::Suspended, 0, 6),
            (Execution::Suspended, 0, 7),
            (Execution::Returned(Value::Integer(6)), 77, 11)
        ]
    );
    Ok(())
}

mod branches;

mod expstate;

mod iteration;

mod switch;

mod unicode;

mod octal;
