//! Error-object views borrow the one native lifetime independently of typed history.
use super::Runner;
use crate::{Execution, Realm, RunnerFailure, Value, VmError};

const fn string_view(runner: &Runner) -> Option<&crate::ByteString> {
    runner.native_error()
}
const fn object_view(runner: &Runner) -> &Value {
    runner.native_error_value()
}

#[test]
fn native_error_views_share_one_owner_and_reset_preserves_typed_history() {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    assert_eq!(object_view(&runner), &Value::Null);
    let program = runner.compile("assert(false);").expect("compile");
    {
        let mut vm = runner.start(&program).expect("frame");
        assert_eq!(vm.resume(10000), Err(VmError::NativeCall));
        let Value::String(object) = vm.native_error_value() else {
            panic!("native assertion raises a string object");
        };
        assert!(std::ptr::eq(
            object,
            vm.native_error().expect("string view")
        ));
        assert_eq!(object.as_bytes(), b"assertion failed");
    }
    assert_eq!(realm.owners(b"assertion failed"), 1);
    let Value::String(object) = object_view(&runner) else {
        panic!("error survives frame destruction");
    };
    assert!(std::ptr::eq(
        object,
        string_view(&runner).expect("same view")
    ));
    let next = runner.compile("return 7;").expect("compile success");
    assert_eq!(
        runner.start(&next).expect("next").resume(10000),
        Ok(Execution::Returned(Value::Integer(7)))
    );
    assert_eq!(realm.owners(b"assertion failed"), 1);
    runner.reset_native_error();
    assert_eq!(object_view(&runner), &Value::Null);
    assert!(string_view(&runner).is_none());
    assert_eq!(realm.owners(b"assertion failed"), 0);
    assert_eq!(
        runner.last_failure(),
        Some(RunnerFailure::Runtime(VmError::NativeCall))
    );
}

#[test]
fn compiler_replaces_error_object_and_policy_refusal_clears_its_string_view() {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let failure = runner.compile("return ; )").expect_err("syntax error");
    assert!(matches!(object_view(&runner), Value::String(_)));
    assert_eq!(runner.last_failure(), Some(RunnerFailure::Compile(failure)));
    runner.compile("return 3;").expect("success retains error");
    assert!(matches!(object_view(&runner), Value::String(_)));
    runner
        .compile("yield 3;")
        .expect_err("unsupported compiler domain");
    assert_eq!(object_view(&runner), &Value::Null);
    assert!(string_view(&runner).is_none());
}
