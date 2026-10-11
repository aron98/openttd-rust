//! `DerefInc` stores its arithmetic target even when native Set returns false.
use crate::{Execution, Realm, Runner, Value};
use std::fmt::Write;

#[test]
fn ignored_compound_set_failure_preserves_native_last_error_and_result() {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let mut output = String::new();
    for source in [
        "const C=3;\nlocal f=function(){this=7;\nreturn this.C+=1;};\nreturn f();",
        "return this.C+=1;",
    ] {
        let main = runner
            .compile_bytes_named(source.as_bytes(), c"compound-store")
            .expect("compile");
        let mut vm = runner.start(&main).expect("start");
        let Execution::Returned(Value::Integer(value)) = vm.resume(10000).expect("execute") else {
            panic!("integer return");
        };
        writeln!(output, "returned 1 {} integer {value}", vm.remaining_ops()).expect("format");
        let bytes = vm
            .native_error()
            .expect("retained original Set error")
            .as_bytes();
        write!(output, "last error string {} ", bytes.len()).expect("format");
        for byte in bytes {
            write!(output, "{byte:02x}").expect("format");
        }
        output.push('\n');
    }
    assert_eq!(
        output,
        super::tests::expected(
            "OTTD_SCRIPT_CLOSURE_COMPOUND_STORE",
            include_str!("compound-store.txt")
        )
    );
}
