//! Success-expecting regressions for the pinned Squirrel 2.2 variadic consumer.
use ottd_script::{Execution, Realm, Runner, Value, Vm, VmError, compile};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn discovery_variadic_count_returns_three() -> Result {
    let program = compile(include_str!(
        "../../../scripts/compat/script-vm/varargs/fixtures/discovery.nut"
    ))?;
    assert_eq!(
        Vm::new(&program)?.resume(10000)?,
        Execution::Returned(Value::Integer(3))
    );
    Ok(())
}

#[test]
fn fixed_parameters_exclude_extra_arguments_from_count() -> Result {
    let program = compile(
        "local f=function(a,...) { return a+vargc*10+vargv[0]+vargv[1]; };\nreturn f(1,2,3);",
    )?;
    assert_eq!(
        Vm::new(&program)?.resume(10000)?,
        Execution::Returned(Value::Integer(26))
    );
    Ok(())
}

#[test]
fn alias_survives_tail_calls_and_fragmented_budget() -> Result {
    let realm = Realm::new();
    let root = realm.default_root()?;
    let leaf = realm.table()?;
    leaf.new_slot(Value::String(realm.string(b"x")), Value::Integer(1))?;
    root.new_slot(b"held", Value::Table(leaf.clone()))?;
    let mut runner = Runner::new(root.clone());
    let program = runner.compile(include_str!(
        "../../../scripts/compat/script-vm/varargs/fixtures/owner-tail.nut"
    ))?;
    let mut vm = runner.start(&program)?;
    assert!(matches!(vm.resume(0)?, Execution::Suspended));
    let mut result = Execution::Suspended;
    for _ in 0..1000 {
        realm.collect_cycles();
        result = vm.resume(2)?;
        if matches!(result, Execution::Returned(_)) {
            break;
        }
    }
    assert_eq!(result, Execution::Returned(Value::Integer(4)));
    assert_eq!(
        leaf.raw_get(Value::String(realm.string(b"x")))?,
        Some(Value::Integer(4))
    );
    assert_eq!(root.raw_get(b"held"), None);
    Ok(())
}

#[test]
fn nested_default_retains_current_vararg_alias() -> Result {
    let program = compile(
        "local t={x=1};\nlocal f=function(...) { local g=function(a=vargv[0]) { return a.x; };\nvargv[0].x=9;\nreturn g; };\nlocal g=f(t);\nreturn g();",
    )?;
    assert_eq!(
        Vm::new(&program)?.resume(10000)?,
        Execution::Returned(Value::Integer(9))
    );
    Ok(())
}

#[test]
fn nested_calls_restore_parent_varargs() -> Result {
    let program = compile(
        "local f=function(...) { local g=function(...) { return vargc+vargv[0]; };\nlocal n=g(7,8);\nreturn n+vargc+vargv[0]; };\nreturn f(3);",
    )?;
    assert_eq!(
        Vm::new(&program)?.resume(10000)?,
        Execution::Returned(Value::Integer(13))
    );
    Ok(())
}

#[test]
fn reached_late_error_preserves_mutation_and_runner_reuse() -> Result {
    let realm = Realm::new();
    let root = realm.default_root()?;
    let leaf = realm.table()?;
    leaf.new_slot(Value::String(realm.string(b"x")), Value::Integer(1))?;
    root.new_slot(b"held", Value::Table(leaf.clone()))?;
    let mut runner = Runner::new(root.clone());
    let program = runner.compile(include_str!(
        "../../../scripts/compat/script-vm/varargs/fixtures/owner-error.nut"
    ))?;
    assert!(runner.start(&program)?.resume(10000).is_err());
    assert_eq!(
        runner.native_error().map(ottd_script::ByteString::as_bytes),
        Some(b"vargv index out of range".as_slice())
    );
    assert_eq!(
        leaf.raw_get(Value::String(realm.string(b"x")))?,
        Some(Value::Integer(9))
    );
    assert_eq!(root.raw_get(b"held"), None);
    let next = runner.compile("return vargc;")?;
    assert_eq!(
        runner.start(&next)?.resume(1000)?,
        Execution::Returned(Value::Integer(0))
    );
    Ok(())
}

#[test]
fn too_few_fixed_arguments_reach_runtime_error() -> Result {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root()?);
    let program = runner.compile("local f=function(a,b,...) { return 0; };\nreturn f(1);")?;
    assert_eq!(
        runner.start(&program)?.resume(10000),
        Err(VmError::NativeCall)
    );
    assert_eq!(
        runner.native_error().map(ottd_script::ByteString::as_bytes),
        Some(b"wrong number of parameters".as_slice())
    );
    Ok(())
}

#[test]
fn fixed_function_vargc_is_zero_after_default_fill() -> Result {
    let program = compile("local f=function(a=7) { return vargc+a; };\nreturn f();")?;
    assert_eq!(
        Vm::new(&program)?.resume(10000)?,
        Execution::Returned(Value::Integer(7))
    );
    Ok(())
}
