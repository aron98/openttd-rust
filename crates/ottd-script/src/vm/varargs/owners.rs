//! Whole passive native owner transcripts including references and collection.
use crate::{Execution, Realm, Runner, Temporary, Value};
use std::fmt::Write;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn value(output: &mut String, value: &Value) -> std::fmt::Result {
    match value {
        Value::Integer(n) => writeln!(output, "integer {n}"),
        Value::Table(_) => writeln!(output, "unsupported 167772192"),
        Value::String(s) => {
            write!(output, "string {} ", s.as_bytes().len())?;
            for byte in s.as_bytes() {
                write!(output, "{byte:02x}")?;
            }
            writeln!(output)
        }
        other => panic!("unexpected fixture value {other:?}"),
    }
}

fn owner(source: &str, expected: &str, environment: &str) -> Result {
    let realm = Realm::new();
    let root = realm.default_root()?;
    let held = realm.table()?;
    held.new_slot(Value::String(realm.string(b"x")), Value::Integer(1))?;
    let watch = held.watch();
    let references = held.0.reference_probe();
    root.new_slot(b"held", Value::Table(held))?;
    let mut runner = Runner::new(root.clone());
    let program = runner.compile_bytes_named(source.as_bytes(), c"varargs-owner")?;
    let mut vm = runner.start(&program)?;
    let mut result = vm.resume(0);
    let mut output = String::new();
    for step in 0..1000 {
        let count = references().unwrap_or(0);
        let alive = count != 0;
        writeln!(
            output,
            "owner {step} {} {} {} {} {} {count}",
            u8::from(result.is_ok()),
            u8::from(matches!(result, Ok(Execution::Suspended))),
            vm.remaining_ops(),
            vm.varargs_stack.len(),
            u8::from(alive)
        )?;
        if let Some(held) = watch() {
            output.push_str("mutation ");
            value(
                &mut output,
                &held
                    .raw_get(Value::String(realm.string(b"x")))?
                    .ok_or("x")?,
            )?;
        }
        match &result {
            Err(_) => {
                output.push_str("error ");
                value(
                    &mut output,
                    &Value::String(vm.native_error().ok_or("native error")?.clone()),
                )?;
                break;
            }
            Ok(Execution::Returned(returned)) => {
                output.push_str("returned ");
                value(&mut output, returned)?;
                break;
            }
            Ok(Execution::Suspended) => {}
        }
        writeln!(
            output,
            "varargs {} {} {}",
            vm.varargs_stack.len(),
            vm.varargs.base,
            vm.varargs.len
        )?;
        for (index, argument) in vm.varargs_stack.iter().enumerate() {
            write!(output, "vararg {index} ")?;
            value(&mut output, argument)?;
        }
        writeln!(output, "collected {}", realm.heap.collect())?;
        result = vm.resume(2);
    }
    assert!(!matches!(result, Ok(Execution::Suspended)));
    drop(vm);
    drop(program);
    drop(result);
    runner.temporary = Temporary::Value(Value::Null);
    if root.raw_get(b"held").is_some() {
        drop(root.delete_slot(b"held")?);
    }
    writeln!(output, "released {} 0", u8::from(watch().is_some()))?;
    writeln!(output, "final collection {}", realm.heap.collect())?;
    writeln!(output, "final alive {}", u8::from(watch().is_some()))?;
    let expected = std::env::var_os(environment)
        .map_or_else(|| Ok(expected.to_owned()), std::fs::read_to_string)?;
    assert_eq!(output, expected);
    Ok(())
}

#[test]
fn return_owner_matches_native_reference_and_cleanup_trace() -> Result {
    owner(
        include_str!("../../../../../scripts/compat/script-vm/varargs/fixtures/owner-return.nut"),
        include_str!("owner-return.txt"),
        "OTTD_SCRIPT_VARARGS_RETURN",
    )
}
#[test]
fn tail_owner_matches_native_reference_and_cleanup_trace() -> Result {
    owner(
        include_str!("../../../../../scripts/compat/script-vm/varargs/fixtures/owner-tail.nut"),
        include_str!("owner-tail.txt"),
        "OTTD_SCRIPT_VARARGS_TAIL",
    )
}
#[test]
fn reached_error_matches_native_reference_and_cleanup_trace() -> Result {
    owner(
        include_str!("../../../../../scripts/compat/script-vm/varargs/fixtures/owner-error.nut"),
        include_str!("owner-error.txt"),
        "OTTD_SCRIPT_VARARGS_ERROR",
    )
}
