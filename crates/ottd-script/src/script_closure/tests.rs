//! Real execution ownership is compared to passive original-VM observations.
use crate::{Execution, Realm, Runner, Temporary, Value};
use std::{fmt::Write, rc::Rc};

pub(super) fn expected(name: &str, embedded: &str) -> String {
    std::env::var_os(name).map_or_else(
        || embedded.into(),
        |path| std::fs::read_to_string(path).expect("native capture"),
    )
}

#[test]
fn returned_capture_owners_release_immediately_or_after_native_cycle_finalization() {
    for (source, cycle, name, fixture) in [
        (
            "local t={key=7};\nreturn function():(t){return t;};",
            false,
            "OTTD_SCRIPT_CLOSURE_ACYCLIC",
            include_str!("acyclic.txt"),
        ),
        (
            "local t={};\nlocal f=function():(t){return t;};\nt.f<-f;\nreturn f;",
            true,
            "OTTD_SCRIPT_CLOSURE_CYCLE",
            include_str!("cycle.txt"),
        ),
        (
            "local t={};\nlocal f=function(a=t){return a;};\nt.f<-f;\nreturn f;",
            true,
            "OTTD_SCRIPT_CLOSURE_DEFAULT_CYCLE",
            include_str!("default-cycle.txt"),
        ),
    ] {
        assert_eq!(ownership_trace(source, cycle), expected(name, fixture));
    }
}

fn ownership_trace(source: &str, cycle: bool) -> String {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let main = runner
        .compile_bytes_named(source.as_bytes(), c"closure-owner")
        .expect("compile");
    let Execution::Returned(Value::ScriptClosure(result)) = runner
        .start(&main)
        .expect("start")
        .resume(10000)
        .expect("execute")
    else {
        panic!("returned closure");
    };
    let closure_watch = result.0.watch();
    let prototype_watch = result.read(|data| Rc::downgrade(&data.prototype));
    let leaf_watch = result.read(|data| {
        match data
            .values()
            .next()
            .expect("capture or default")
            .value(&realm)
        {
            Value::Table(table) => table.watch(),
            _ => panic!("table capture"),
        }
    });
    let mut output = String::new();
    writeln!(
        output,
        "owners {} {} {} {}",
        main.data.0.references(),
        result.0.references(),
        result.read(|data| Rc::strong_count(&data.prototype)),
        result.read(|data| data
            .values()
            .next()
            .expect("capture or default")
            .edge()
            .expect("edge")
            .references())
    )
    .expect("format");
    drop(main);
    writeln!(
        output,
        "main released proto {}",
        result.read(|data| Rc::strong_count(&data.prototype))
    )
    .expect("format");
    drop(result);
    let Temporary::Value(Value::ScriptClosure(temporary)) = runner.temporary.as_ref() else {
        panic!("temporary closure");
    };
    writeln!(
        output,
        "result released closure {}",
        temporary.0.references()
    )
    .expect("format");
    runner.temporary.set(Temporary::Value(Value::Null));
    writeln!(
        output,
        "temporary released {} {} {}",
        u8::from(closure_watch().is_some()),
        u8::from(prototype_watch.upgrade().is_some()),
        u8::from(leaf_watch().is_some())
    )
    .expect("format");
    if cycle {
        writeln!(output, "collected {}", realm.heap.collect()).expect("format");
    }
    writeln!(
        output,
        "final {} {} {}",
        u8::from(closure_watch().is_some()),
        u8::from(prototype_watch.upgrade().is_some()),
        u8::from(leaf_watch().is_some())
    )
    .expect("format");
    output
}

#[test]
fn failed_symbol_capture_retains_earlier_edges_in_unpublished_native_orphan() {
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    let table = realm.table().expect("table");
    let watch = table.watch();
    root.new_slot(b"held", Value::Table(table)).expect("slot");
    let mut runner = Runner::new(root.clone());
    let main = runner
        .compile_bytes_named(
            b"return function():(held,missing){return held;};",
            c"failed-capture",
        )
        .expect("compile");
    let mut frame = runner.start(&main).expect("start");
    assert_eq!(frame.resume(10000), Err(crate::VmError::MissingIndex));
    let debt = frame.remaining_ops();
    let message = frame.native_error().expect("actual native error").clone();
    drop(frame);
    drop(root.delete_slot(b"held").expect("delete"));
    drop(main);
    runner.temporary.set(Temporary::Value(Value::Null));
    let mut output = format!(
        "result 0 {debt} {}\n",
        std::str::from_utf8(message.as_bytes()).expect("error bytes")
    );
    writeln!(output, "held after error {}", u8::from(watch().is_some())).expect("format");
    writeln!(output, "collected {}", realm.heap.collect()).expect("format");
    writeln!(
        output,
        "held after collection {}",
        u8::from(watch().is_some())
    )
    .expect("format");
    assert_eq!(
        output,
        expected("OTTD_SCRIPT_CLOSURE_ORPHAN", include_str!("orphan.txt"))
    );
}

#[test]
fn prototype_view_retains_code_without_retaining_closure_or_realm() {
    let realm = Realm::new();
    let realm_watch = realm.downgrade();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let main = runner
        .compile_bytes_named(b"return function(){return 7;};", c"prototype-view")
        .expect("compile");
    let Execution::Returned(Value::ScriptClosure(result)) = runner
        .start(&main)
        .expect("start")
        .resume(10000)
        .expect("execute")
    else {
        panic!("closure result");
    };
    let closure_watch = result.0.watch();
    let prototype_watch = result.read(|data| Rc::downgrade(&data.prototype));
    let mut output = String::new();
    writeln!(
        output,
        "before view {} {}",
        result.0.references(),
        result.read(|data| Rc::strong_count(&data.prototype))
    )
    .expect("format");
    let view = result.prototype();
    writeln!(
        output,
        "with view {} {}",
        result.0.references(),
        result.read(|data| Rc::strong_count(&data.prototype))
    )
    .expect("format");
    drop(main);
    drop(result);
    runner.temporary.set(Temporary::Value(Value::Null));
    writeln!(
        output,
        "owners released {} {} {}",
        u8::from(closure_watch().is_some()),
        Rc::strong_count(&view.0),
        view.instructions().len()
    )
    .expect("format");
    drop(runner);
    drop(realm);
    assert!(realm_watch.upgrade().is_none());
    drop(view);
    writeln!(
        output,
        "view released {}",
        u8::from(prototype_watch.upgrade().is_some())
    )
    .expect("format");
    assert_eq!(
        output,
        expected("OTTD_SCRIPT_CLOSURE_VIEW", include_str!("view.txt"))
    );
}
