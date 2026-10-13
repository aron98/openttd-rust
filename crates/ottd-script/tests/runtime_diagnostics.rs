//! Source-defined opcode failures retain the exact object raised by the native VM.
use ottd_script::{Execution, Realm, Runner, Value, VmError};

#[test]
fn opcode_errors_preserve_native_bytes_and_typed_failures() {
    for (source, typed, expected) in [
        ("return 7/0;", VmError::DivisionByZero, "division by zero"),
        ("return 7%0;", VmError::ModuloByZero, "modulo by zero"),
        (
            "return null-1;",
            VmError::OperandType,
            "arith op - on between 'null' and 'integer'",
        ),
        (
            "return true&1;",
            VmError::OperandType,
            "bitwise op between 'bool' and 'integer'",
        ),
        (
            "local x=null;\nreturn -x;",
            VmError::OperandType,
            "attempt to negate a null",
        ),
        (
            "return ~true;",
            VmError::OperandType,
            "attempt to perform a bitwise op on a bool",
        ),
        (
            "return true<1;",
            VmError::OperandType,
            "comparison between 'bool' and '1'",
        ),
        (
            "foreach(x in 1){}",
            VmError::OperandType,
            "cannot iterate integer",
        ),
        (
            "local x=null;\nx++;",
            VmError::OperandType,
            "arith op + on between 'null' and 'integer'",
        ),
        (
            "local x={a=null};\nx.a+=1;",
            VmError::OperandType,
            "arith op + on between 'null' and 'integer'",
        ),
        (
            "return {}<1;",
            VmError::UnsupportedTableOrdering,
            "comparison between 'table' and '1'",
        ),
        (
            "return []<true;",
            VmError::UnsupportedArrayOrdering,
            "comparison between 'array' and 'bool'",
        ),
    ] {
        let realm = Realm::new();
        let root = realm.default_root().expect("root");
        let mut runner = Runner::new(root.clone());
        let program = runner
            .compile(&format!("before<-7;\n{source}"))
            .expect("defined source");
        let mut vm = runner.start(&program).expect("frame");
        assert_eq!(vm.resume(0), Ok(Execution::Suspended));
        assert_eq!(vm.resume(10000), Err(typed), "{source}");
        assert_eq!(
            vm.native_error().map(ottd_script::ByteString::as_bytes),
            Some(expected.as_bytes()),
            "{source}"
        );
        assert_eq!(vm.resume(1), Err(VmError::Finished));
        drop(vm);
        assert_eq!(root.raw_get(b"before"), Some(Value::Integer(7)));
    }
}
