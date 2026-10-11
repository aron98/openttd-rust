//! Exact passive bare-VM registration and native closure ownership witnesses.
use crate::{NativeClosure, NativeFunction, Realm, Runner, Value};
use std::fmt::Write;
fn closure(root: &super::RootEnvironment) -> NativeClosure {
    let Some(Value::NativeClosure(value)) = root.raw_get(b"print") else {
        panic!("native closure")
    };
    value
}
fn registration_trace() -> String {
    let realm = Realm::new();
    let root = realm.default_root().expect("default root");
    let configured = super::RootEnvironment::from_table(root.table());
    assert!(configured.table.same_identity(&root.table));
    let runner = Runner::new(configured);
    let mut output = String::new();
    let mut cursor = 0;
    while let Some((next, key, value)) = root.table.next(cursor) {
        cursor = next;
        let Value::String(key) = key else {
            panic!("registration key")
        };
        write!(
            output,
            "slot {next} {} ",
            std::str::from_utf8(key.as_bytes()).expect("name")
        )
        .expect("format");
        match value {
            Value::NativeClosure(value) => {
                writeln!(output, "unsupported 134218240").expect("format");
                assert_eq!(value.environment(), Value::Null);
                write!(
                    output,
                    "metadata {} {} {} 16777217",
                    std::str::from_utf8(value.name().as_bytes()).expect("name"),
                    value.parameter_check(),
                    value.outer_value_count()
                )
                .expect("format");
                for mask in value.type_checks() {
                    write!(output, " {mask}").expect("format");
                }
                output.push('\n');
            }
            Value::Integer(value) => writeln!(output, "integer {value}").expect("format"),
            Value::String(value) => {
                write!(output, "string {} ", value.as_bytes().len()).expect("format");
                for byte in value.as_bytes() {
                    write!(output, "{byte:02x}").expect("format");
                }
                output.push('\n');
            }
            value => panic!("unexpected root value {value:?}"),
        }
    }
    let old = closure(&root);
    let watch = old.watch();
    let copy = root.table.shallow_clone().expect("clone");
    let Some(Value::NativeClosure(copied)) = copy
        .raw_get(Value::String(realm.string(b"print")))
        .expect("lookup")
    else {
        panic!("copied closure")
    };
    writeln!(
        output,
        "clone alias {}",
        u8::from(old.same_identity(&copied))
    )
    .expect("format");
    drop(copied);
    let friend = runner.friend().expect("friend registration");
    let current = closure(&root);
    writeln!(
        output,
        "friend root {}",
        u8::from(root.table.same_identity(&friend.root.table))
    )
    .expect("format");
    writeln!(
        output,
        "friend replaced {}",
        u8::from(!old.same_identity(&current))
    )
    .expect("format");
    drop(old);
    writeln!(output, "old clone-held {}", u8::from(watch().is_some())).expect("format");
    drop(copy);
    writeln!(output, "old released {}", u8::from(watch().is_some())).expect("format");
    let current_watch = current.watch();
    drop(current);
    drop(friend);
    writeln!(
        output,
        "current root-held {}",
        u8::from(current_watch().is_some())
    )
    .expect("format");
    root.delete_slot(b"print").expect("delete");
    writeln!(
        output,
        "current deleted {}",
        u8::from(current_watch().is_some())
    )
    .expect("format");
    output
}
#[test]
#[ignore = "Requires a freshly executed target-native full registration trace"]
fn native_root_registration() {
    let path = std::env::var("OTTD_SCRIPT_ROOT_REGISTRATION").expect("fresh native capture");
    assert_eq!(
        registration_trace(),
        std::fs::read_to_string(path).expect("native trace")
    );
}
#[test]
fn default_roots_retain_real_metadata_and_friend_identities() {
    let realm = Realm::new();
    let root = realm.default_root().expect("default root");
    assert_eq!(root.table.len(), 8);
    for (name, function, arity, masks) in [
        (b"assert".as_slice(), NativeFunction::Assert, 2, &[][..]),
        (b"print", NativeFunction::Print, 2, &[][..]),
        (b"array", NativeFunction::Array, -2, &[-1, 6][..]),
        (b"type", NativeFunction::Type, 2, &[][..]),
    ] {
        let Some(Value::NativeClosure(value)) = root.raw_get(name) else {
            panic!("closure")
        };
        assert_eq!(value.name().as_bytes(), name);
        assert_eq!(value.function(), function);
        assert_eq!(value.outer_value_count(), 0);
        assert_eq!(value.environment(), Value::Null);
        assert_eq!(value.parameter_check(), arity);
        assert_eq!(value.type_checks(), masks);
    }
    let old = closure(&root);
    let watch = old.watch();
    let runner = Runner::new(root.clone());
    let friend = runner.friend().expect("friend");
    assert!(root.table.same_identity(&friend.root.table));
    assert!(!old.same_identity(&closure(&root)));
    drop(old);
    assert!(watch().is_none());
    let foreign = Realm::new();
    let foreign_root = foreign.default_root().expect("foreign root");
    assert_eq!(
        root.set_existing(b"absent", Value::NativeClosure(closure(&foreign_root))),
        Err(crate::VmError::RealmMismatch)
    );
    let empty = realm.empty_root();
    assert!(empty.table.is_empty());
}

#[test]
fn root_cycle_teardown_releases_native_closure_storage() {
    let realm = Realm::new();
    let heap = std::rc::Rc::downgrade(&realm.heap);
    let root = realm.default_root().expect("root");
    let root_watch = root.table.watch();
    let print = closure(&root);
    let closure_watch = print.watch();
    drop(print);
    root.new_slot(b"self", Value::Table(root.table()))
        .expect("cycle");
    let mut output = String::new();
    writeln!(
        output,
        "root teardown before {} {}",
        u8::from(root_watch().is_some()),
        u8::from(closure_watch().is_some())
    )
    .expect("format");
    drop(root);
    drop(realm);
    writeln!(
        output,
        "root teardown after {} {}",
        u8::from(root_watch().is_some()),
        u8::from(closure_watch().is_some())
    )
    .expect("format");
    assert!(heap.upgrade().is_none());
    let native = std::env::var("OTTD_SCRIPT_ROOT_TEARDOWN").ok().map_or_else(
        || include_str!("../../tests/root-teardown.txt").to_owned(),
        |path| std::fs::read_to_string(path).expect("native trace"),
    );
    assert_eq!(output, native);
}
