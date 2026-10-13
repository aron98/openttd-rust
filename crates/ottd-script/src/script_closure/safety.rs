//! Checked malformed instructions retain effects already performed by real frames.
use crate::{Execution, Instruction, Realm, Runner, Value, VmError, heap::Object};
use std::{fmt::Write, rc::Rc};

fn corrupt(main: &crate::Program, child: bool, opcode: u8, edit: impl FnOnce(&mut Instruction)) {
    main.data.0.mutate(|object| {
        let Object::ScriptClosure(closure) = object else {
            panic!("script closure");
        };
        let mut proto = Rc::get_mut(&mut closure.prototype).expect("sole prototype owner");
        if child {
            proto = Rc::get_mut(proto.children.first_mut().expect("child"))
                .expect("sole child prototype owner");
        }
        edit(
            proto
                .instructions
                .iter_mut()
                .find(|i| i.opcode == opcode)
                .expect("source instruction"),
        );
    });
}

#[test]
fn malformed_closure_index_preserves_prior_root_mutation() {
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    let mut runner = Runner::new(root.clone());
    let main = runner
        .compile("mark<-1;\nreturn function(){return 7;};")
        .expect("compile");
    corrupt(&main, false, 0x30, |i| i.arg1 = i32::MAX);
    let mut vm = runner.start(&main).expect("start");
    assert_eq!(vm.resume(10000), Err(VmError::InvalidBytecode));
    assert_eq!(root.raw_get(b"mark"), Some(Value::Integer(1)));
    assert_eq!(vm.resume(10000), Err(VmError::Finished));
}

#[test]
fn malformed_outer_index_releases_nested_frames_after_prior_body_effect() {
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    let mut runner = Runner::new(root.clone());
    let main = runner
        .compile("local x=7;\nlocal f=function():(x){mark<-2;\nreturn x;};\nreturn f();")
        .expect("compile");
    corrupt(&main, true, 0x1b, |i| i.arg1 = i32::MAX);
    assert_eq!(
        runner.start(&main).expect("start").resume(10000),
        Err(VmError::InvalidBytecode)
    );
    assert_eq!(root.raw_get(b"mark"), Some(Value::Integer(2)));
    let next = runner.compile("return mark;").expect("next compile");
    assert_eq!(
        runner.start(&next).expect("next start").resume(10000),
        Ok(Execution::Returned(Value::Integer(2)))
    );
}

#[test]
fn malformed_call_arguments_do_not_enter_body() {
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    let mut runner = Runner::new(root.clone());
    let main = runner
        .compile("mark<-1;\nlocal f=function(){mark=2;};\nreturn f();")
        .expect("compile");
    corrupt(&main, false, 0x06, |i| i.arg3 = 0);
    assert_eq!(
        runner.start(&main).expect("start").resume(10000),
        Err(VmError::InvalidBytecode)
    );
    assert_eq!(root.raw_get(b"mark"), Some(Value::Integer(1)));
}

#[test]
fn malformed_child_return_retains_completed_body_mutation() {
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    let mut runner = Runner::new(root.clone());
    let main = runner
        .compile("local f=function(){mark<-2;\nreturn 7;};\nreturn f();")
        .expect("compile");
    corrupt(&main, true, 0x13, |i| i.arg1 = i32::MAX);
    assert_eq!(
        runner.start(&main).expect("start").resume(10000),
        Err(VmError::InvalidBytecode)
    );
    assert_eq!(root.raw_get(b"mark"), Some(Value::Integer(2)));
}

#[test]
fn realm_teardown_releases_closure_cycles_and_unpublished_orphans() {
    for source in [
        "local t={};\nlocal f=function():(t){return t;};\nt.f<-f;\nreturn f;",
        "local t={};\nreturn function():(t,missing){return t;};",
    ] {
        let realm = Realm::new();
        let watch = realm.downgrade();
        let mut runner = Runner::new(realm.default_root().expect("root"));
        let main = runner.compile(source).expect("compile");
        let result = runner.start(&main).expect("start").resume(10000);
        drop(main);
        drop(result);
        drop(runner);
        drop(realm);
        assert!(watch.upgrade().is_none());
    }
}

#[test]
fn rejected_capture_assignment_preserves_rhs_compiler_publication() {
    let realm = Realm::new();
    let failed = realm
        .compile_bytes_named(
            b"local x=1;\nlocal f=function():(x){x=function(){const Published=7;\nreturn 2;};};",
            c"publication",
        )
        .is_err();
    let mut output = format!("compile failed {}\n", u8::from(failed));
    match realm.constant_bytes(b"Published") {
        Some(crate::realm::constants::Binding::Scalar(value)) => {
            let Value::Integer(value) = value.value() else {
                panic!("integer constant");
            };
            writeln!(output, "published 1 integer {value}").expect("format");
        }
        Some(crate::realm::constants::Binding::Enum(_)) => panic!("scalar constant"),
        None => output.push_str("published 0 null\n"),
    }
    assert_eq!(
        output,
        super::tests::expected(
            "OTTD_SCRIPT_CLOSURE_PUBLICATION",
            include_str!("publication.txt")
        )
    );
}
