//! Persistent slot extraction preserves externally callable const and borrow contracts.
use ottd_script::{ByteString, Realm, RootEnvironment, Runner, Value, VmError};

const fn configured(root: RootEnvironment) -> Runner {
    Runner::new(root)
}
const fn object(runner: &Runner) -> &Value {
    runner.native_error_value()
}
const fn message(runner: &Runner) -> Option<&ByteString> {
    runner.native_error()
}
const CREATE: fn(RootEnvironment) -> Runner = Runner::new;
const ERROR: fn(&Runner) -> &Value = Runner::native_error_value;
const MESSAGE: fn(&Runner) -> Option<&ByteString> = Runner::native_error;

#[test]
fn public_const_wrappers_and_function_pointers_borrow_the_retained_error() {
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    for create in [configured, CREATE] {
        let mut runner = create(root.clone());
        let program = runner.compile("assert(false);").expect("source");
        assert_eq!(
            runner.start(&program).expect("frame").resume(10000),
            Err(VmError::NativeCall)
        );
        let Value::String(retained) = object(&runner) else {
            panic!("native string error");
        };
        assert_eq!(retained.as_bytes(), b"assertion failed");
        assert!(std::ptr::eq(object(&runner), ERROR(&runner)));
        assert!(std::ptr::eq(
            retained,
            message(&runner).expect("const view")
        ));
        assert!(std::ptr::eq(
            retained,
            MESSAGE(&runner).expect("function pointer view")
        ));
    }
}
