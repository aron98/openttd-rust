//! Compile diagnostics preserve typed errors and lazy source/realm effects.
use ottd_script::{CompileError, CompileErrorKind, Execution, Realm, Runner, RunnerFailure, Value};

#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "The input is intentionally invalid Squirrel table syntax, not a Rust format string"
)]
#[test]
fn native_messages_do_not_replace_public_typed_errors() {
    for (source, offset, message) in [
        ("return {a:1};", 9, "expected '='"),
        ("return {\"a\"=1};", 8, "expected 'IDENTIFIER'"),
        ("return );", 7, "expression expected"),
        ("return type(7,);", 14, "expression expected, found ')'"),
        ("break;", 0, "'break' has to be in a loop block"),
        ("return \"x", 9, "unfinished string"),
    ] {
        let realm = Realm::new();
        let mut runner = Runner::new(realm.default_root().expect("root"));
        let expected = CompileError {
            offset,
            kind: CompileErrorKind::ExpectedToken,
        };
        assert_eq!(runner.compile(source).expect_err("reject"), expected);
        assert_eq!(
            ottd_script::compile(source).expect_err("public reject"),
            expected
        );
        assert_eq!(realm.compile(source).expect_err("realm reject"), expected);
        assert_eq!(
            runner.last_failure(),
            Some(RunnerFailure::Compile(expected))
        );
        assert_eq!(
            runner.native_error().expect("diagnostic").as_bytes(),
            message.as_bytes()
        );
        assert_eq!(
            expected.to_string(),
            format!("ExpectedToken at byte {offset}")
        );
    }
}

#[test]
fn first_failure_preserves_lazy_admission_and_native_order() {
    for (source, message) in [
        (b"1=);".as_slice(), "can't assign expression"),
        (b"local a=1;\na<-);".as_slice(), "expression expected"),
        (
            b"local x=1;\nlocal f=function():(x){x=);};",
            "expression expected",
        ),
        (b"return ); \xff", "expression expected"),
        ("return ); €".as_bytes(), "expression expected"),
        (
            "local s=\"€\";\nreturn );".as_bytes(),
            "expression expected",
        ),
    ] {
        let realm = Realm::new();
        let mut runner = Runner::new(realm.default_root().expect("root"));
        let error = runner.compile_bytes(source).expect_err("reject");
        assert!(matches!(
            error.kind,
            CompileErrorKind::ExpectedToken | CompileErrorKind::UnsupportedSyntax
        ));
        assert_eq!(
            runner.native_error().expect("diagnostic").as_bytes(),
            message.as_bytes()
        );
    }
}

#[test]
fn policy_and_safety_failures_do_not_reuse_native_messages() {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    for source in ["€".as_bytes(), b"\xff", b"class A {}", &vec![b' '; 65537]] {
        runner
            .compile("return );")
            .expect_err("seed native message");
        runner
            .compile_bytes(source)
            .expect_err("policy or unsupported");
        assert!(runner.native_error().is_none());
    }
}

#[test]
fn successful_unicode_and_nul_terminated_compile_preserve_error_and_effects() {
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    let mut runner = Runner::new(root.clone());
    runner
        .compile("const kept=7;\nreturn );")
        .expect_err("partial publication");
    let diagnostic = runner.native_error().expect("diagnostic").clone();
    for source in [
        "/* €漢 */\n// ő\nreturn kept;".as_bytes(),
        b"return kept;\0\xff",
    ] {
        let program = runner
            .compile_bytes_named(source, c"unicode")
            .expect("compile");
        assert_eq!(runner.native_error(), Some(&diagnostic));
        let mut vm = runner.start(&program).expect("start");
        assert_eq!(vm.resume(0), Ok(Execution::Suspended));
        assert_eq!(vm.resume(10000), Ok(Execution::Returned(Value::Integer(7))));
    }
    assert_eq!(root.raw_get(b"kept"), None);
    runner
        .compile("enum E {a=1};\nreturn E.missing;")
        .expect_err("missing constant");
    assert_eq!(
        runner
            .native_error()
            .expect("dynamic diagnostic")
            .as_bytes(),
        b"invalid constant [E.missing]"
    );
}
