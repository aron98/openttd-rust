//! Known metadata must not masquerade as implemented or catchable native code.
use super::{NativeClosure, NativeFunction, NativeRegistry};
use crate::{Realm, Runner, Value, VmError};

#[test]
fn known_defaults_refuse_calls_before_arity_without_a_native_error() {
    // Given: all 66 default registrations, manually installed only by this test.
    for row in NativeRegistry::ALL.iter().flat_map(|r| r.registrations()) {
        if let NativeFunction::Default(function) = row.function() {
            for arguments in ["", "1", "1, 2, 3"] {
                let realm = Realm::new();
                let root = realm.default_root().expect("bare root");
                root.new_slot(
                    b"f",
                    Value::NativeClosure(NativeClosure::registered(&realm, row).expect("row")),
                )
                .expect("test binding");
                let mut runner = Runner::new(root.clone());
                let program = runner
                    .compile(&format!(
                        "try {{ f({arguments}); }} catch(e) {{ caught <- true; }} return 0;"
                    ))
                    .expect("source");
                // When: actual CALL executes with valid or mismatched native arities.
                let result = runner.start(&program).expect("frame").resume(1000);
                // Then: the infrastructure refusal escapes catch without inventing last_error.
                assert_eq!(result, Err(VmError::UnsupportedNativeFunction(function)));
                assert_eq!(root.raw_get(b"caught"), None);
                assert_eq!(runner.native_error_value(), &Value::Null);
            }
        }
    }
}

#[test]
fn unsupported_default_preserves_prior_error_and_argument_effects() {
    // Given: a known default plus a genuine retained script error.
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    let row = NativeRegistry::Array
        .registrations()
        .iter()
        .find(|row| row.name() == b"append")
        .expect("append");
    let NativeFunction::Default(function) = row.function() else {
        panic!("default")
    };
    root.new_slot(
        b"f",
        Value::NativeClosure(NativeClosure::registered(&realm, row).expect("row")),
    )
    .expect("binding");
    root.new_slot(b"effect", Value::Integer(0)).expect("effect");
    let mut runner = Runner::new(root.clone());
    let program = runner.compile("try { throw \"prior\\0error\"; } catch(e) {} try { f(effect = 7); } catch(e) { caught <- true; } return 0;").expect("source");
    // When: evaluation reaches the unsupported callback after evaluating its argument.
    let result = runner.start(&program).expect("frame").resume(1000);
    // Then: the prior error and evaluated effect survive; catch does not swallow the refusal.
    assert_eq!(result, Err(VmError::UnsupportedNativeFunction(function)));
    assert_eq!(root.raw_get(b"effect"), Some(Value::Integer(7)));
    assert_eq!(root.raw_get(b"caught"), None);
    assert_eq!(
        runner.native_error().expect("prior error").as_bytes(),
        b"prior\0error"
    );
}
