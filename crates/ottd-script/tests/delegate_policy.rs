//! Defined native hook/default executions remain required, explicitly unported behavior.
use ottd_script::{Realm, Runner, Value, VmError};

const DEFINED_BOUNDARIES: &[(&str, &str, VmError)] = &[
    (
        "get-hook-success",
        include_str!("required-metahooks/get-hook-success.nut"),
        VmError::UnsupportedMetamethod,
    ),
    (
        "get-hook-owns-receiver",
        include_str!("required-metahooks/get-hook-owns-receiver.nut"),
        VmError::UnsupportedMetamethod,
    ),
    (
        "get-failure-index-replacement",
        include_str!("required-metahooks/get-failure-index-replacement.nut"),
        VmError::UnsupportedMetamethod,
    ),
    (
        "set-hook-success",
        include_str!("required-metahooks/set-hook-success.nut"),
        VmError::UnsupportedMetamethod,
    ),
    (
        "newslot-throw-falls-back",
        include_str!("required-metahooks/newslot-throw-falls-back.nut"),
        VmError::UnsupportedMetamethod,
    ),
    (
        "delete-throw-falls-back",
        include_str!("required-metahooks/delete-throw-falls-back.nut"),
        VmError::UnsupportedMetamethod,
    ),
    (
        "clone-throw-ignored",
        include_str!("required-metahooks/clone-throw-ignored.nut"),
        VmError::UnsupportedMetamethod,
    ),
    (
        "hook-catch-local",
        include_str!("required-metahooks/hook-catch-local.nut"),
        VmError::UnsupportedMetamethod,
    ),
    (
        "hook-budget-debt",
        include_str!("required-metahooks/hook-budget-debt.nut"),
        VmError::UnsupportedMetamethod,
    ),
    (
        "arithmetic-hook",
        include_str!("required-metahooks/arithmetic-hook.nut"),
        VmError::UnsupportedMetamethod,
    ),
    (
        "comparison-hook",
        include_str!("required-metahooks/comparison-hook.nut"),
        VmError::UnsupportedMetamethod,
    ),
    (
        "type-hook",
        include_str!("required-metahooks/type-hook.nut"),
        VmError::UnsupportedMetamethod,
    ),
    (
        "call-hook",
        include_str!("required-metahooks/call-hook.nut"),
        VmError::UnsupportedMetamethod,
    ),
    (
        "string-hook",
        include_str!("required-metahooks/string-hook.nut"),
        VmError::UnsupportedMetamethod,
    ),
    (
        "default-table-len",
        include_str!("required-metahooks/default-table-len.nut"),
        VmError::UnsupportedRuntimeValue,
    ),
    (
        "default-table-rawget",
        include_str!("required-metahooks/default-table-rawget.nut"),
        VmError::UnsupportedRuntimeValue,
    ),
    (
        "default-table-rawset",
        include_str!("required-metahooks/default-table-rawset.nut"),
        VmError::UnsupportedRuntimeValue,
    ),
    (
        "default-table-rawin",
        include_str!("required-metahooks/default-table-rawin.nut"),
        VmError::UnsupportedRuntimeValue,
    ),
];

#[test]
fn defined_hook_and_default_bodies_stop_at_typed_policy_boundaries() {
    for &(name, source, expected) in DEFINED_BOUNDARIES {
        let realm = Realm::new();
        let mut runner = Runner::new(realm.default_root().expect("root"));
        let program = runner.compile(source).expect("defined native syntax");
        let mut vm = runner.start(&program).expect("frame");
        assert_eq!(vm.resume(10000), Err(expected), "{name}");
        assert_eq!(
            vm.native_error_value(),
            &Value::Null,
            "no manufactured native error: {name}"
        );
    }
}

#[test]
fn reached_lookup_and_symbol_capture_guards_preserve_historical_error_objects() {
    for operation in [
        "return t.missing;",
        "this=t;\nreturn function():(missing){return missing;};",
        "this=t;\nreturn function():(len){return len;};",
    ] {
        let realm = Realm::new();
        let root = realm.default_root().expect("root");
        let mut runner = Runner::new(root.clone());
        let source = format!(
            "held<-[7];\ntry{{throw held;}}catch(e){{}}\nlocal d={{_get=function(k){{return 9;}}}};\nlocal t=delegate d:{{}};\ntry{{{operation}}}catch(e){{return 99;}}"
        );
        let program = runner.compile(&source).expect("source");
        let error = root.raw_get(b"held");
        assert_eq!(error, None);
        let mut vm = runner.start(&program).expect("frame");
        let expected = if operation.contains("(len)") {
            VmError::UnsupportedRuntimeValue
        } else {
            VmError::UnsupportedMetamethod
        };
        assert_eq!(vm.resume(10000), Err(expected));
        assert_eq!(Some(vm.native_error_value().clone()), root.raw_get(b"held"));
    }
}
