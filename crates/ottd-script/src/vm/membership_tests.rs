//! Raw membership owns successful lookup results and leaves the temporary on misses.
use super::{Instruction, Temporary, Value, Vm};
use crate::Program;

#[test]
fn successful_membership_retains_value_and_missing_lookup_keeps_it() {
    let program = Program::from_parts(4, vec![], vec![]).expect("program");
    let realm = program.data.0.realm.clone();
    let table = realm.table().expect("container");
    let child = realm.table().expect("child");
    let watch = child.watch();
    let key = Value::String(realm.string(b"hit"));
    table
        .new_slot(key.clone(), Value::Table(child))
        .expect("slot");
    let mut vm = Vm::new(&program).expect("frame");
    vm.write(1, Value::Table(table.clone())).expect("receiver");
    vm.write(2, key.clone()).expect("key");
    vm.runner
        .get_mut()
        .temporary
        .set(Temporary::Value(Value::Integer(17)));
    let instruction = Instruction {
        opcode: 0x29,
        arg0: 3,
        arg1: 1,
        arg2: 2,
        arg3: 0,
    };
    assert_eq!(vm.step(instruction), Ok(None));
    assert_eq!(vm.register(3), Ok(Value::Bool(true)));
    drop(table.delete_slot(key).expect("remove last container owner"));
    assert!(watch().is_some());
    vm.write(2, Value::String(realm.string(b"missing")))
        .expect("missing key");
    assert_eq!(vm.step(instruction), Ok(None));
    assert_eq!(vm.register(3), Ok(Value::Bool(false)));
    assert!(watch().is_some());
    let _value = vm.store_temporary(Value::Null).expect("replace temporary");
    assert!(watch().is_none());
}
