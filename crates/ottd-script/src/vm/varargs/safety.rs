//! Reached undefined-original refusals are distinct from defined script errors.
use crate::{Execution, Instruction, Program, Realm, Runner, Value, Vm, VmError};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn failed_variadic_tail_replacement_refuses_only_after_prior_effects() -> Result {
    for callee in [
        "function(a,b){return 0;}",
        "function(a,b,...){return 0;}",
        "function(){return 0;}",
    ] {
        let realm = Realm::new();
        let root = realm.default_root()?;
        let mut runner = Runner::new(root.clone());
        let source = format!(
            "mark<-0;\nlocal bad={callee};\nlocal f=function(...) :(bad) {{mark=7;\nreturn bad(mark=9);}};\nreturn f(1);"
        );
        let program = runner.compile(&source)?;
        let mut vm = runner.start(&program)?;
        let mut result = Ok(Execution::Suspended);
        for _ in 0..1000 {
            result = vm.resume(2);
            if !matches!(result, Ok(Execution::Suspended)) {
                break;
            }
        }
        assert_eq!(result, Err(VmError::UndefinedNativeVarargsTailCall));
        assert_eq!(root.raw_get(b"mark"), Some(Value::Integer(9)));
        assert!(vm.native_error().is_none());
        assert!(vm.varargs_stack.is_empty());
        assert_eq!(vm.resume(10000), Err(VmError::Finished));
        drop(vm);
        let next = runner.compile("return mark+vargc;")?;
        assert_eq!(
            runner.start(&next)?.resume(10000)?,
            Execution::Returned(Value::Integer(9))
        );
    }
    Ok(())
}

#[test]
fn wrong_arity_is_defined_without_a_nonempty_tail_replacement() -> Result {
    for source in [
        "local bad=function(a){return a;};\nlocal f=function(...) :(bad) {return bad();};\nreturn f();",
        "local bad=function(a){return a;};\nlocal f=function(...) :(bad) {local x=bad();\nreturn x;};\nreturn f(1);",
        "local f=function(...) {return assert(false);};\nreturn f(1);",
    ] {
        let realm = Realm::new();
        let mut runner = Runner::new(realm.default_root()?);
        let program = runner.compile(source)?;
        assert_eq!(
            runner.start(&program)?.resume(10000),
            Err(VmError::NativeCall)
        );
        assert!(runner.native_error().is_some());
    }
    Ok(())
}

#[test]
fn unreachable_failed_tail_is_not_a_source_rejection() -> Result {
    let program = crate::compile(
        "local bad=function(a){return a;};\nlocal f=function(...) :(bad) {if(false) return bad();\nreturn vargc;};\nreturn f(1);",
    )?;
    assert_eq!(
        Vm::new(&program)?.resume(10000)?,
        Execution::Returned(Value::Integer(1))
    );
    Ok(())
}

#[test]
fn dropping_suspended_nested_frames_releases_unnamed_owners() -> Result {
    let realm = Realm::new();
    let root = realm.default_root()?;
    let leaf = realm.table()?;
    let watch = leaf.watch();
    root.new_slot(b"held", Value::Table(leaf))?;
    let mut runner = Runner::new(root);
    let program = runner.compile("local child=function(...) {while(true){}};\nlocal f=function(...) :(child) {delete ::held;\nchild(vargv[0]);};\nf(held);")?;
    let mut vm = runner.start(&program)?;
    assert_eq!(vm.resume(100), Ok(Execution::Suspended));
    assert_eq!(vm.varargs_stack.len(), 2);
    assert!(watch().is_some());
    assert_eq!(realm.heap.collect(), 0);
    drop(vm);
    assert!(watch().is_none());
    Ok(())
}

#[test]
fn returned_array_alias_outlives_the_vararg_frame() -> Result {
    let realm = Realm::new();
    let root = realm.default_root()?;
    let leaf = realm.array(Vec::new())?;
    root.new_slot(b"held", Value::Array(leaf.clone()))?;
    let mut runner = Runner::new(root);
    let program = runner
        .compile("local f=function(...) {delete ::held;\nreturn vargv[0];};\nreturn f(held);")?;
    let result = runner.start(&program)?.resume(10000)?;
    assert_eq!(result, Execution::Returned(Value::Array(leaf)));
    Ok(())
}

#[test]
fn nonfinite_index_is_rejected_only_when_conversion_is_reached() -> Result {
    let realm = Realm::new();
    let root = realm.default_root()?;
    root.new_slot(b"index", Value::Float(f32::INFINITY.to_bits()))?;
    let mut runner = Runner::new(root);
    let program = runner.compile("local f=function(...) {return vargv[index];};\nreturn f(1);")?;
    assert_eq!(
        runner.start(&program)?.resume(10000),
        Err(VmError::UnsupportedIndexConversion)
    );
    let empty = runner.compile("local f=function(...) {return vargv[index];};\nreturn f();")?;
    assert_eq!(
        runner.start(&empty)?.resume(10000),
        Err(VmError::NativeCall)
    );
    assert_eq!(
        runner.native_error().map(crate::ByteString::as_bytes),
        Some(b"the function doesn't have var args".as_slice())
    );
    Ok(())
}

#[test]
fn malformed_vararg_operands_are_checked() -> Result {
    for i in [
        Instruction {
            opcode: 0x1c,
            arg0: 0,
            arg1: 1,
            arg2: 0,
            arg3: 0,
        },
        Instruction {
            opcode: 0x1c,
            arg0: 0,
            arg1: 0,
            arg2: 1,
            arg3: 0,
        },
        Instruction {
            opcode: 0x1d,
            arg0: 0,
            arg1: -1,
            arg2: 0,
            arg3: 0,
        },
        Instruction {
            opcode: 0x1d,
            arg0: 0,
            arg1: 0,
            arg2: 0,
            arg3: 1,
        },
    ] {
        let program = Program::from_parts(1, Vec::new(), vec![i])?;
        assert_eq!(
            Vm::new(&program)?.resume(10000),
            Err(VmError::InvalidBytecode)
        );
    }
    Ok(())
}

#[test]
fn deep_unnamed_stack_preserves_native_u16_lookup_base_and_count_cleanup() -> Result {
    let source = include_str!("../../../../../scripts/compat/script-vm/varargs/fixtures/deep.nut");
    let program = crate::compile(source)?;
    let mut vm = Vm::new(&program)?;
    assert_eq!(
        vm.resume(100_000)?,
        Execution::Returned(Value::Integer(329))
    );
    assert!(vm.varargs_stack.is_empty());
    Ok(())
}
