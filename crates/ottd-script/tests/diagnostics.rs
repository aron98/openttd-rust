//! Defined VM errors retain native diagnostic bytes independently of typed failures.
use ottd_script::{Execution, Realm, Runner, VmError};

#[test]
fn diagnostic_paths_preserve_typed_errors_and_prior_effects() {
    for (operation, typed, message) in [
        (
            "return t[null];",
            VmError::MissingIndex,
            "the index 'null' does not exist",
        ),
        (
            "t.absent=9;",
            VmError::MissingIndex,
            "the index 'absent' does not exist",
        ),
        (
            "return delete t.absent;",
            VmError::MissingIndex,
            "the index 'absent' does not exist",
        ),
        ("return clone 7;", VmError::OperandType, "cloning a integer"),
        (
            "return delete 7 .absent;",
            VmError::OperandType,
            "attempt to delete a slot from a integer",
        ),
        (
            "t[null]<-9;",
            VmError::NullIndex,
            "null cannot be used as index",
        ),
        (
            "t.absent+=9;",
            VmError::MissingIndex,
            "the index 'absent' does not exist",
        ),
    ] {
        let realm = Realm::new();
        let root = realm.default_root().expect("root");
        let mut runner = Runner::new(root.clone());
        let source = format!("held<-7;\nlocal t={{}};\n{operation}");
        let program = runner
            .compile_bytes_named(source.as_bytes(), c"diagnostic")
            .expect("compile");
        let mut vm = runner.start(&program).expect("start");
        assert_eq!(vm.resume(0), Ok(Execution::Suspended));
        assert_eq!(vm.resume(10000), Err(typed), "{source}");
        assert_eq!(
            vm.native_error().expect("native diagnostic").as_bytes(),
            message.as_bytes(),
            "{source}"
        );
        drop(vm);
        assert_eq!(
            root.table()
                .raw_get(ottd_script::Value::String(realm.string(b"held")))
                .expect("slot"),
            Some(ottd_script::Value::Integer(7))
        );
    }
}

#[test]
fn diagnostic_precision_is_fifty_codepoints_with_embedded_nul() {
    for (key, expected) in [
        ("a".repeat(51), "a".repeat(50).into_bytes()),
        (
            format!("{}€z", "a".repeat(49)),
            format!("{}€", "a".repeat(49)).into_bytes(),
        ),
        (format!("{}z", "€".repeat(50)), "€".repeat(50).into_bytes()),
        ("a\\0b".to_owned(), b"a\0b".to_vec()),
        (
            format!("{}\\0z", "a".repeat(49)),
            ["a".repeat(49).as_bytes(), b"\0"].concat(),
        ),
    ] {
        let realm = Realm::new();
        let mut runner = Runner::new(realm.default_root().expect("root"));
        let source = format!("local t={{}};\nreturn t[\"{key}\"];");
        let program = runner
            .compile_bytes_named(source.as_bytes(), c"diagnostic")
            .expect("compile");
        let mut vm = runner.start(&program).expect("start");
        assert_eq!(vm.resume(10000), Err(VmError::MissingIndex));
        let message = [b"the index '".as_slice(), &expected, b"' does not exist"].concat();
        assert_eq!(
            vm.native_error().expect("native diagnostic").as_bytes(),
            message
        );
    }
}

#[test]
fn index_float_diagnostic_uses_fourteen_significant_digits() {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let program = runner
        .compile_bytes_named(b"local t={};\nreturn t[1.234567];", c"diagnostic")
        .expect("compile");
    let mut vm = runner.start(&program).expect("start");
    assert_eq!(vm.resume(10000), Err(VmError::MissingIndex));
    assert_eq!(
        vm.native_error().expect("native diagnostic").as_bytes(),
        b"the index '1.2345670461655' does not exist"
    );
}

#[test]
fn missing_capture_precision_preserves_typed_error_effects_and_reuse() {
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    let mut runner = Runner::new(root.clone());
    let source = format!(
        "held<-7;\nlocal prior=\"retained\";\nlocal f=function():(prior,{}) {{return 1;}};",
        "a".repeat(60)
    );
    let program = runner.compile(&source).expect("compile");
    let mut vm = runner.start(&program).expect("start");
    assert_eq!(vm.resume(0), Ok(Execution::Suspended));
    assert_eq!(vm.resume(10000), Err(VmError::MissingIndex));
    assert_eq!(
        vm.native_error().expect("native diagnostic").as_bytes(),
        format!("the index '{}' does not exist", "a".repeat(50)).as_bytes()
    );
    assert_eq!(vm.resume(1), Err(VmError::Finished));
    drop(vm);
    assert_eq!(root.raw_get(b"held"), Some(ottd_script::Value::Integer(7)));
    realm.collect_cycles();
    let next = runner
        .compile("return held;")
        .expect("compile after failure");
    assert_eq!(
        runner.start(&next).expect("reuse runner").resume(10000),
        Ok(Execution::Returned(ottd_script::Value::Integer(7)))
    );
}
