//! First-class root escape, suspension, clone alias and cycle ownership.
use crate::{Execution, Realm, Runner, Value};
use std::fmt::Write;
#[test]
fn escaped_roots_and_native_closures_match_original_owners() {
    ownership(false);
    ownership(true);
}
fn ownership(reverse: bool) {
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    let root_watch = root.table.watch();
    let Some(Value::NativeClosure(print)) = root.raw_get(b"print") else {
        panic!("print")
    };
    let print_watch = print.watch();
    drop(print);
    let mut runner = Runner::new(root);
    let program = runner
        .compile(if reverse {
            include_str!(
                "../../../../scripts/compat/script-vm/tables/fixtures/roots/escaped-reverse.nut"
            )
        } else {
            include_str!("../../../../scripts/compat/script-vm/tables/fixtures/roots/escaped.nut")
        })
        .expect("source");
    let mut vm = runner.start(&program).expect("vm");
    assert_eq!(vm.resume(2), Ok(Execution::Suspended));
    let mut output = String::new();
    writeln!(
        output,
        "suspended {} {}",
        vm.remaining_ops(),
        vm.instruction_pointer()
    )
    .expect("format");
    vm.runner.get_mut().root.set(realm.empty_root());
    writeln!(
        output,
        "frame collect {} {} {}",
        realm.heap.collect(),
        u8::from(root_watch().is_some()),
        u8::from(print_watch().is_some())
    )
    .expect("format");
    let Execution::Returned(Value::Array(returned)) = vm.resume(100).expect("returned") else {
        panic!("array")
    };
    writeln!(output, "returned {}", vm.remaining_ops()).expect("format");
    drop(vm);
    drop(program);
    escaped_aliases(&returned, &realm, &mut output, reverse);
    let program = runner.compile("return 1;").expect("next");
    runner
        .start(&program)
        .expect("vm")
        .resume(10000)
        .expect("replace temporary");
    writeln!(
        output,
        "host collect {} {} {}",
        realm.heap.collect(),
        u8::from(root_watch().is_some()),
        u8::from(print_watch().is_some())
    )
    .expect("format");
    drop(returned);
    writeln!(
        output,
        "unrooted before {} {}",
        u8::from(root_watch().is_some()),
        u8::from(print_watch().is_some())
    )
    .expect("format");
    writeln!(
        output,
        "unrooted collect {} {} {}",
        realm.heap.collect(),
        u8::from(root_watch().is_some()),
        u8::from(print_watch().is_some())
    )
    .expect("format");
    let native = std::env::var(if reverse {
        "OTTD_SCRIPT_ROOT_OWNERSHIP_REVERSE"
    } else {
        "OTTD_SCRIPT_ROOT_OWNERSHIP"
    })
    .ok()
    .map_or_else(
        || {
            if reverse {
                include_str!("../../tests/root-ownership-reverse.txt")
            } else {
                include_str!("../../tests/root-ownership.txt")
            }
            .to_owned()
        },
        |path| std::fs::read_to_string(path).expect("native trace"),
    );
    assert_eq!(output, native);
}

fn escaped_aliases(returned: &crate::Array, realm: &Realm, output: &mut String, reverse: bool) {
    let Some(Value::Table(original)) = returned.get(i64::from(reverse)) else {
        panic!("root")
    };
    let Some(Value::Table(copy)) = returned.get(i64::from(!reverse)) else {
        panic!("copy")
    };
    let Some(Value::Table(own)) = copy
        .raw_get(Value::String(realm.string(b"self")))
        .expect("self")
    else {
        panic!("self")
    };
    let Some(Value::NativeClosure(left)) = original
        .raw_get(Value::String(realm.string(b"print")))
        .expect("print")
    else {
        panic!("print")
    };
    let Some(Value::NativeClosure(right)) = copy
        .raw_get(Value::String(realm.string(b"print")))
        .expect("print")
    else {
        panic!("print")
    };
    writeln!(
        output,
        "escaped aliases {} {} {}",
        u8::from(!original.same_identity(&copy)),
        u8::from(own.same_identity(&original)),
        u8::from(left.same_identity(&right))
    )
    .expect("format");
    drop((original, copy, own, left, right));
}
