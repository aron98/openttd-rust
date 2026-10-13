//! Native hidden-node and removed-result string retention.
use crate::{Realm, Value};
use std::fmt::Write;
#[test]
fn hidden_and_removed_values_match_native_storage_session() {
    let realm = Realm::new();
    let source = realm.table().expect("source");
    source
        .set_existing(Value::Null, Value::String(realm.string(b"hidden-value")))
        .expect("native null set");
    let copy = source.shallow_clone().expect("copy");
    let mut output = String::new();
    writeln!(output, "counts {} {}", source.len(), copy.len()).expect("format");
    writeln!(
        output,
        "hidden before {}",
        u8::from(realm.owners(b"hidden-value") > 0)
    )
    .expect("format");
    drop(source);
    writeln!(
        output,
        "hidden after-source {}",
        u8::from(realm.owners(b"hidden-value") > 0)
    )
    .expect("format");
    let key = Value::String(realm.string(b"key"));
    copy.new_slot(key.clone(), Value::String(realm.string(b"removed-value")))
        .expect("slot");
    let removed = copy.delete_slot(key).expect("removed");
    writeln!(output, "removed count {}", copy.len()).expect("format");
    writeln!(
        output,
        "removed held {}",
        u8::from(realm.owners(b"removed-value") > 0)
    )
    .expect("format");
    drop(removed);
    writeln!(
        output,
        "removed released {}",
        u8::from(realm.owners(b"removed-value") > 0)
    )
    .expect("format");
    let array = realm
        .array(vec![Value::String(realm.string(b"array-value"))])
        .expect("array");
    let copy = array.shallow_clone().expect("array copy");
    writeln!(
        output,
        "array distinct {}",
        u8::from(!array.same_identity(&copy))
    )
    .expect("format");
    drop(array);
    writeln!(
        output,
        "array source-dropped {}",
        u8::from(realm.owners(b"array-value") > 0)
    )
    .expect("format");
    drop(copy);
    writeln!(
        output,
        "array copy-dropped {}",
        u8::from(realm.owners(b"array-value") > 0)
    )
    .expect("format");
    assert_eq!(
        output,
        super::native(
            "OTTD_SCRIPT_TABLE_STORAGE",
            include_str!("../../../tests/table-storage.txt")
        )
    );
}

#[test]
fn late_failure_retains_native_delete_and_clone_effects() {
    let realm = Realm::new();
    let root = realm.empty_root();
    let mut runner = crate::Runner::new(root.clone());
    let program = runner
        .compile(include_str!(
            "../../../../../scripts/compat/script-vm/tables/fixtures/delete-effects.nut"
        ))
        .expect("source");
    let mut vm = runner.start(&program).expect("frame");
    assert_eq!(vm.resume(10000), Err(crate::VmError::MissingIndex));
    let mut output = format!("result 0 {}\n", vm.remaining_ops());
    drop(vm);
    for name in ["held", "copy"] {
        let Value::Table(table) = root.raw_get(name.as_bytes()).expect("root") else {
            panic!("table")
        };
        writeln!(output, "{name} count {}", table.len()).expect("format");
        let mut cursor = 0;
        while let Some((next, key, value)) = table.next(cursor) {
            let Value::String(key) = key else {
                panic!("string")
            };
            let Value::Integer(value) = value else {
                panic!("integer")
            };
            write!(output, "cursor {next} string {} ", key.as_bytes().len()).expect("format");
            for byte in key.as_bytes() {
                write!(output, "{byte:02x}").expect("format");
            }
            writeln!(output, "\ninteger {value}").expect("format");
            cursor = next;
        }
    }
    assert_eq!(
        output,
        super::native("OTTD_SCRIPT_DELETE_EFFECTS", delete_effects_fixture())
    );
}

fn delete_effects_fixture() -> &'static str {
    if cfg!(all(
        target_pointer_width = "64",
        target_endian = "little",
        target_vendor = "apple"
    )) {
        include_str!("../../../tests/delete-effects.txt")
    } else if cfg!(all(
        target_pointer_width = "64",
        target_endian = "little",
        target_os = "linux",
        target_env = "gnu"
    )) {
        include_str!("../../../tests/delete-effects-gnu.txt")
    } else {
        panic!("delete-effects fixture requires an admitted native hash ABI")
    }
}

#[test]
fn bad_delete_destination_preserves_completed_slot_removal() {
    for destination in [254, 255] {
        let realm = Realm::new();
        let root = realm.empty_root();
        let mut runner = crate::Runner::new(root.clone());
        let source = runner
            .compile("held<-{a=7};\nreturn delete held.a;")
            .expect("source");
        let mut instructions = source.instructions();
        instructions
            .iter_mut()
            .find(|i| i.opcode == 0x0c)
            .expect("delete")
            .arg0 = destination;
        let program = crate::Program::freeze(crate::ProgramData {
            stack_size: source.stack_size(),
            literals: source.literals(),
            instructions,
            realm: realm.clone(),
        });
        assert_eq!(
            runner.start(&program).expect("vm").resume(100),
            Err(crate::VmError::InvalidBytecode)
        );
        let Value::Table(table) = root.raw_get(b"held").expect("root") else {
            panic!("table")
        };
        assert_eq!(table.raw_get(Value::String(realm.string(b"a"))), Ok(None));
        assert!(table.is_empty());
    }
}
