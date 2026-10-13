//! Lifetime observations pair weak handles with genuine native table sessions.
use crate::{Execution, Realm, Runner, Value};
use std::fmt::Write;
#[test]
fn retained_owners_match_native_session() {
    owner_session(
        include_str!("../../../../scripts/compat/script-vm/tables/fixtures/owner.nut"),
        &native(
            "OTTD_SCRIPT_TABLE_OWNER",
            include_str!("../../tests/tables-owner.txt"),
        ),
    );
}
fn owner_session(source: &str, native: &str) {
    let realm = Realm::new();
    let root = realm.empty_root();
    let mut runner = Runner::new(root.clone());
    let program = runner.compile(source).expect("native source");
    let result = runner
        .start(&program)
        .expect("frame")
        .resume(10000)
        .expect("result");
    let Execution::Returned(Value::Table(table)) = result else {
        panic!("table result")
    };
    let watch = table.watch();
    let mut output = String::new();
    let Value::Table(root_table) = root.raw_get(b"held").expect("held") else {
        panic!("table root")
    };
    writeln!(
        output,
        "same {}",
        u8::from(table.same_identity(&root_table))
    )
    .expect("format");
    drop(root_table);
    table
        .set_existing(Value::String(realm.string(b"key")), Value::Integer(9))
        .expect("store");
    drop(table);
    writeln!(
        output,
        "alive result-dropped {}",
        u8::from(watch().is_some())
    )
    .expect("format");
    root.set_existing(b"held", Value::Null).expect("clear");
    writeln!(output, "alive root-cleared {}", u8::from(watch().is_some())).expect("format");
    let next = runner.compile("return 7;").expect("compile");
    writeln!(output, "alive compiled {}", u8::from(watch().is_some())).expect("format");
    runner
        .start(&next)
        .expect("frame")
        .resume(10000)
        .expect("return");
    writeln!(
        output,
        "alive temp-replaced {}",
        u8::from(watch().is_some())
    )
    .expect("format");
    let result = runner
        .start(&program)
        .expect("frame")
        .resume(10000)
        .expect("result");
    let Execution::Returned(Value::Table(table)) = result else {
        panic!("table result")
    };
    let watch = table.watch();
    drop(table);
    let reader = runner.compile("return held.key;").expect("reader");
    let result = runner
        .start(&reader)
        .expect("frame")
        .resume(10000)
        .expect("result");
    let Execution::Returned(Value::Integer(value)) = result else {
        panic!("integer result")
    };
    writeln!(output, "persistent integer {value}").expect("format");
    writeln!(output, "alive root-only {}", u8::from(watch().is_some())).expect("format");
    root.set_existing(b"held", Value::Null).expect("clear");
    writeln!(output, "alive released {}", u8::from(watch().is_some())).expect("format");
    assert_eq!(output, native);
}

#[test]
fn suspended_host_mutation_matches_native_session() {
    mutation_session(
        false,
        include_str!("../../../../scripts/compat/script-vm/tables/fixtures/mutation.nut"),
        &native(
            "OTTD_SCRIPT_TABLE_MUTATION",
            include_str!("../../tests/tables-mutation.txt"),
        ),
    );
}
fn mutation_session(remove: bool, source: &str, native: &str) {
    let realm = Realm::new();
    let root = realm.empty_root();
    let table = realm.table().expect("table");
    for key in if remove {
        (0..32).collect()
    } else {
        vec![0, 4, 8, 12]
    } {
        table
            .new_slot(Value::Integer(key), Value::Integer(key))
            .expect("slot");
    }
    root.new_slot(b"held", Value::Table(table.clone()))
        .expect("root");
    let mut runner = Runner::new(root.clone());
    let program = runner.compile(source).expect("source");
    let mut vm = runner.start(&program).expect("frame");
    let mut output = String::new();
    for step in 0..200 {
        match vm.resume(2).expect("resume") {
            Execution::Suspended => {
                writeln!(
                    output,
                    "suspend {} {}",
                    vm.remaining_ops(),
                    vm.instruction_pointer()
                )
                .expect("format");
                if step == 5 {
                    if remove {
                        for key in 2..32 {
                            table.delete_slot(Value::Integer(key)).expect("delete");
                        }
                    } else {
                        for key in [16, 20, 24, 28, 32] {
                            table
                                .new_slot(
                                    Value::Integer(key),
                                    Value::Integer(key.checked_add(1).expect("small fixture key")),
                                )
                                .expect("mutate");
                        }
                    }
                    root.set_existing(b"held", Value::Table(realm.table().expect("replacement")))
                        .expect("rebind");
                    writeln!(output, "host-mutated-and-rebound").expect("format");
                }
            }
            Execution::Returned(value) => {
                let Value::String(bytes) = value else {
                    panic!("string result")
                };
                let mut rendered = format!("string {} ", bytes.as_bytes().len());
                for byte in bytes.as_bytes() {
                    write!(rendered, "{byte:02x}").expect("format");
                }
                writeln!(output, "return {} {rendered}", vm.remaining_ops()).expect("format");
                break;
            }
        }
    }
    assert_eq!(output, native);
}

fn native(variable: &str, frozen: &str) -> String {
    std::env::var_os(variable).map_or_else(
        || frozen.to_owned(),
        |path| std::fs::read_to_string(path).expect("fresh native session"),
    )
}
#[test]
fn cloned_owner_matches_native_session() {
    owner_session(
        include_str!("../../../../scripts/compat/script-vm/tables/fixtures/clone-owner.nut"),
        &native(
            "OTTD_SCRIPT_CLONE_OWNER",
            include_str!("../../tests/clone-owner.txt"),
        ),
    );
}
#[test]
fn deleted_owner_matches_native_session() {
    owner_session(
        include_str!("../../../../scripts/compat/script-vm/tables/fixtures/delete-owner.nut"),
        &native(
            "OTTD_SCRIPT_DELETE_OWNER",
            include_str!("../../tests/delete-owner.txt"),
        ),
    );
}
#[test]
fn suspended_removal_matches_native_session() {
    mutation_session(
        true,
        include_str!("../../../../scripts/compat/script-vm/tables/fixtures/delete-mutation.nut"),
        &native(
            "OTTD_SCRIPT_DELETE_MUTATION",
            include_str!("../../tests/delete-mutation.txt"),
        ),
    );
}
mod storage;
