//! Byte-string iteration through the public compiler, root and runner APIs.
use ottd_script::{Execution, Realm, Runner, Value, Vm, compile};
use std::error::Error;
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

#[test]
fn ascii_count() -> Result {
    let program = compile("local n=0; foreach(c in \"abc\") n+=1; return n;")?;
    assert_eq!(
        Vm::new(&program)?.resume(1000)?,
        Execution::Returned(Value::Integer(3))
    );
    Ok(())
}

#[test]
fn byte_offsets_and_values() -> Result {
    let program = compile("local n=0; foreach(i,c in \"abc\") n+=i+c; return n;")?;
    assert_eq!(
        Vm::new(&program)?.resume(1000)?,
        Execution::Returned(Value::Integer(297))
    );
    Ok(())
}

#[test]
fn empty_string_exhausts() -> Result {
    let program = compile("local n=0; foreach(c in \"\") n+=1; return n;")?;
    assert_eq!(
        Vm::new(&program)?.resume(1000)?,
        Execution::Returned(Value::Integer(0))
    );
    Ok(())
}

#[test]
fn foreign_realm_host_bytes_include_nul_and_high_bytes() -> Result {
    let realm = Realm::new();
    let root = realm.empty_root();
    root.new_slot(b"text", Value::String(Realm::new().string(&[0, 128, 255])))?;
    let mut runner = Runner::new(root);
    let program = runner.compile("local a=[]; foreach(i,c in text) { a=[i,c]; } return a;")?;
    let Execution::Returned(Value::Array(array)) = runner.start(&program)?.resume(1000)? else {
        return Err("expected returned byte pair".into());
    };
    assert_eq!(array.get(0), Some(Value::Integer(2)));
    assert_eq!(
        array.get(1),
        Some(Value::Integer(i64::from(std::ffi::c_char::from_ne_bytes(
            [255]
        ))))
    );
    Ok(())
}

#[test]
fn suspended_local_rebinding_preserves_cursor() -> Result {
    let program = compile(
        "local text=\"abcd\"; local n=0; foreach(c in text) { n+=c; text=\"xy\"; } return n;",
    )?;
    let mut vm = Vm::new(&program)?;
    assert_eq!(vm.resume(0)?, Execution::Suspended);
    let result = loop {
        let result = vm.resume(2)?;
        if result != Execution::Suspended {
            break result;
        }
    };
    assert_eq!(result, Execution::Returned(Value::Integer(218)));
    Ok(())
}

#[test]
fn suspended_root_rebinding_keeps_evaluated_receiver() -> Result {
    let realm = Realm::new();
    let root = realm.empty_root();
    root.new_slot(b"text", Value::String(realm.string(b"ab")))?;
    let mut runner = Runner::new(root.clone());
    let program =
        runner.compile("local n=0; foreach(c in text) { n+=c; text=\"z\"; } return n;")?;
    let mut vm = runner.start(&program)?;
    assert_eq!(vm.resume(0)?, Execution::Suspended);
    let result = loop {
        let result = vm.resume(2)?;
        if result != Execution::Suspended {
            break result;
        }
    };
    assert_eq!(result, Execution::Returned(Value::Integer(195)));
    assert_eq!(
        root.raw_get(b"text"),
        Some(Value::String(realm.string(b"z")))
    );
    Ok(())
}
