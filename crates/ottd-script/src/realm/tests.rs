//! Reference counts come from the independently executed native session.
use super::Realm;
use crate::{Execution, Program, Value, Vm};
use std::fmt::Write;
fn refs(realm: &Realm, label: &str, output: &mut String) -> std::fmt::Result {
    let count = realm
        .pool
        .borrow()
        .get(b"pool-shared".as_slice())
        .map_or(0, std::rc::Weak::strong_count);
    writeln!(output, "refs {label} {count}")
}
fn render(value: &Value) -> String {
    match value {
        Value::String(bytes) => format!(
            "string {} {}",
            bytes.as_bytes().len(),
            bytes
                .as_bytes()
                .iter()
                .fold(String::new(), |mut output, byte| {
                    write!(output, "{byte:02x}").expect("String formatting is infallible");
                    output
                })
        ),
        Value::Array(_) => "unsupported 134217792".to_owned(),
        Value::Null => "null".to_owned(),
        Value::Integer(n) => format!("integer {n}"),
        Value::Float(bits) => format!("float {bits}"),
        Value::Bool(b) => format!("bool {}", u8::from(*b)),
    }
}
fn sliced(
    vm: &mut Vm<'_>,
    realm: &Realm,
    output: &mut String,
) -> Result<Value, Box<dyn std::error::Error>> {
    for credit in [0, 2, 2, 2, 2, 2, 100] {
        match vm.resume(credit) {
            Ok(Execution::Suspended) => {
                writeln!(
                    output,
                    "suspend {} {}",
                    vm.remaining_ops(),
                    vm.instruction_pointer()
                )?;
                refs(realm, "suspended", output)?;
            }
            Ok(Execution::Returned(value)) => {
                writeln!(output, "return {} {}", vm.remaining_ops(), render(&value))?;
                refs(realm, "returned", output)?;
                return Ok(value);
            }
            Err(_) => {
                writeln!(output, "runtime_error {}", vm.remaining_ops())?;
                refs(realm, "error", output)?;
                return Ok(Value::Null);
            }
        }
    }
    Err("unexpected nontermination".into())
}
fn first_string(program: &Program) -> Result<&super::ByteString, Box<dyn std::error::Error>> {
    match program.literals().first() {
        Some(Value::String(value)) => Ok(value),
        _ => Err("missing string literal".into()),
    }
}
#[test]
fn realm_lifetimes_match_native_when_programs_and_runner_are_released()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: native source bytes and the retained real session output.
    let literal =
        include_str!("../../../../scripts/compat/script-vm/fixtures/strings_realm_literal.nut");
    let dynamic_source =
        include_str!("../../../../scripts/compat/script-vm/fixtures/strings_realm_dynamic.nut");
    let error_source =
        include_str!("../../../../scripts/compat/script-vm/fixtures/strings_realm_error.nut");
    let realm = Realm::new();
    let mut output = String::new();
    // When: compile, resume and release owners in the native session's order.
    refs(&realm, "initial", &mut output)?;
    let first = realm.compile(literal)?;
    refs(&realm, "first", &mut output)?;
    let second = realm.compile(literal)?;
    refs(&realm, "second", &mut output)?;
    let dynamic = realm.compile(dynamic_source)?;
    refs(&realm, "dynamic", &mut output)?;
    writeln!(
        output,
        "same_literal {}",
        u8::from(first_string(&first)?.same_identity(first_string(&second)?))
    )?;
    drop(first);
    refs(&realm, "drop_first", &mut output)?;
    let mut runner = Vm::new(&dynamic)?;
    let result = sliced(&mut runner, &realm, &mut output)?;
    let Value::String(bytes) = &result else {
        return Err("missing returned string".into());
    };
    writeln!(
        output,
        "same_runtime {}",
        u8::from(bytes.same_identity(first_string(&second)?))
    )?;
    drop(runner);
    refs(&realm, "drop_runner", &mut output)?;
    drop(second);
    refs(&realm, "drop_second", &mut output)?;
    drop(dynamic);
    refs(&realm, "drop_dynamic", &mut output)?;
    writeln!(output, "retained {}", render(&result))?;
    drop(result);
    refs(&realm, "drop_result", &mut output)?;
    let error = realm.compile(error_source)?;
    let mut runner = Vm::new(&error)?;
    let _result = sliced(&mut runner, &realm, &mut output)?;
    drop(runner);
    drop(error);
    refs(&realm, "drop_error", &mut output)?;
    // Then: every reference count, debt/IP and returned byte agrees with native.
    assert_eq!(output, include_str!("../../tests/frames/strings_realm.txt"));
    assert!(realm.pool.borrow().is_empty());
    Ok(())
}

#[test]
fn independent_suspended_owners_match_native_when_one_runner_is_dropped()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: two realms and two suspended VMs sharing one prototype.
    let source =
        include_str!("../../../../scripts/compat/script-vm/fixtures/strings_realm_literal.nut");
    let realm = Realm::new();
    let independent = Realm::new();
    let program = realm.compile(source)?;
    let foreign = independent.compile(source)?;
    let mut output = String::new();
    writeln!(
        output,
        "separate_identity {}",
        u8::from(!first_string(&program)?.same_identity(first_string(&foreign)?))
    )?;
    drop(foreign);
    refs(&realm, "program", &mut output)?;
    let mut first = Vm::new(&program)?;
    let mut second = Vm::new(&program)?;
    // When: suspend both after loading, drop one, return the other's owned value.
    for vm in [&mut first, &mut second] {
        assert_eq!(vm.resume(2)?, Execution::Suspended);
        writeln!(
            output,
            "suspend {} {}",
            vm.remaining_ops(),
            vm.instruction_pointer()
        )?;
        refs(&realm, "loaded", &mut output)?;
    }
    drop(first);
    refs(&realm, "drop_first_runner", &mut output)?;
    let Execution::Returned(result) = second.resume(100)? else {
        return Err("missing return".into());
    };
    writeln!(
        output,
        "return {} {}",
        second.remaining_ops(),
        render(&result)
    )?;
    refs(&realm, "returned", &mut output)?;
    drop(second);
    refs(&realm, "drop_second_runner", &mut output)?;
    drop(program);
    refs(&realm, "drop_program", &mut output)?;
    writeln!(output, "retained {}", render(&result))?;
    drop(result);
    refs(&realm, "drop_result", &mut output)?;
    // Then: native references, debt, IP and retained bytes match exactly.
    assert_eq!(
        output,
        include_str!("../../tests/frames/strings_parallel.txt")
    );
    assert!(realm.pool.borrow().is_empty());
    Ok(())
}

#[test]
fn intern_keys_are_released_when_the_last_value_drops() {
    // Given: one long-lived realm reused for distinct transient values.
    let realm = Realm::new();
    // When: each live value's last owner is released.
    for index in 0_u32..1024 {
        let bytes = index.to_le_bytes();
        let value = realm.string(&bytes);
        assert_eq!(realm.pool.borrow().len(), 1);
        drop(value);
        // Then: weak keys do not accumulate in the persistent realm.
        assert!(realm.pool.borrow().is_empty());
    }
}

#[test]
fn parts_constructor_reinterns_foreign_strings_before_native_identity_equality()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: equal byte values with distinct live realm identities.
    let a = Realm::new().string(b"a\0b");
    let b = Realm::new().string(b"a\0b");
    assert!(!a.same_identity(&b));
    // When: hand-authored literals cross the checked Program boundary.
    let program = Program::from_parts(2, vec![Value::String(a), Value::String(b)], vec![])?;
    let [Value::String(first), Value::String(second)] = program.literals() else {
        return Err("missing literals".into());
    };
    // Then: both belong to the program's identity domain, with all bytes retained.
    assert!(first.same_identity(second));
    assert_eq!(first.as_bytes(), b"a\0b");
    Ok(())
}

#[test]
fn failed_arithmetic_keeps_native_temporary_until_vm_drop() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: concatenation stores a dynamic string in native VM temp_reg before failure.
    let source =
        include_str!("../../../../scripts/compat/script-vm/fixtures/strings_failure_frame.nut");
    let realm = Realm::new();
    let program = realm.compile(source)?;
    let mut runner = Vm::new(&program)?;
    let mut output = String::new();
    // When: execution fails after ARITH but before RETURN can replace temp_reg.
    let result = sliced(&mut runner, &realm, &mut output)?;
    // Then: the real native trace retains one temporary owner after frame cleanup.
    let native = include_str!("../../tests/frames/strings_terminal.txt");
    let prefix = native
        .split("refs drop_error_closure")
        .next()
        .ok_or("missing native boundary")?;
    assert!(native.contains("refs drop_error_closure 1\nclosed\n"));
    assert_eq!(output, prefix);
    drop(result);
    drop(runner);
    drop(program);
    assert!(realm.pool.borrow().is_empty());
    Ok(())
}

#[test]
fn failed_compilation_releases_literals_before_recompiling_in_same_realm()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a compiler failure after interning a complete literal.
    let bad =
        include_str!("../../../../scripts/compat/script-vm/fixtures/strings_compile_failure.nut");
    let good =
        include_str!("../../../../scripts/compat/script-vm/fixtures/strings_realm_literal.nut");
    let realm = Realm::new();
    let mut output = String::new();
    refs(&realm, "initial", &mut output)?;
    // When: the same realm is reused after failure and then released again.
    assert!(realm.compile(bad).is_err());
    writeln!(output, "compile_error")?;
    refs(&realm, "failed_compile", &mut output)?;
    let program = realm.compile(good)?;
    refs(&realm, "recompiled", &mut output)?;
    drop(program);
    refs(&realm, "released", &mut output)?;
    // Then: native failure/recompile reference counts match without leaked keys.
    assert_eq!(
        output,
        include_str!("../../tests/frames/strings_compile_failure.txt")
    );
    assert!(realm.pool.borrow().is_empty());
    Ok(())
}
