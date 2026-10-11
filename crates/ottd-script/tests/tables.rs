//! Native table execution boundary.
use ottd_script::{Execution, Value, Vm, compile};

#[test]
fn tables_compile_execute_and_iterate_scalar_keys() {
    let source = "local t={a=3,[1]=4,[true]=5,[1.0]=6};\nlocal alias=t;\nalias.a=7;\nt[2]<-8;\nlocal sum=0;\nforeach(k,v in t) sum+=v;\nreturn sum;";
    let program = compile(source).expect("native-valid table source");
    let mut vm = Vm::new(&program).expect("valid frame");
    assert_eq!(vm.resume(1000), Ok(Execution::Returned(Value::Integer(30))));
}

#[test]
fn formerly_unsupported_dot_chain_reads_root_table() {
    let realm = ottd_script::Realm::new();
    let root = realm.empty_root();
    let table = realm.table().expect("table");
    table
        .new_slot(Value::String(realm.string(b"x")), Value::Integer(17))
        .expect("slot");
    root.new_slot(b"score", Value::Table(table)).expect("root");
    let mut runner = ottd_script::Runner::new(root);
    let program = runner
        .compile("return ::score.x;")
        .expect("native field fixture");
    assert_eq!(
        runner.start(&program).expect("frame").resume(100),
        Ok(Execution::Returned(Value::Integer(17)))
    );
}

#[test]
fn null_set_has_native_invisible_empty_node_semantics() {
    let source = include_str!("../../../scripts/compat/script-vm/tables/fixtures/null-set.nut");
    let program = compile(source).expect("native source");
    assert_eq!(
        Vm::new(&program).expect("frame").resume(100),
        Ok(Execution::Returned(Value::Integer(7)))
    );
}

#[test]
fn returned_table_keeps_its_realm_after_execution_owners_drop() {
    let table = {
        let program = compile("return {key=42};").expect("source");
        let mut vm = Vm::new(&program).expect("frame");
        let Execution::Returned(Value::Table(table)) = vm.resume(100).expect("return") else {
            panic!("table result")
        };
        table
    };
    let foreign = ottd_script::Realm::new();
    assert_eq!(
        table.raw_get(Value::String(foreign.string(b"key"))),
        Ok(Some(Value::Integer(42)))
    );
}

#[test]
fn aggregate_graph_edges_and_foreign_roots_remain_explicit_boundaries() {
    use ottd_script::{Realm, VmError};
    let realm = Realm::new();
    let table = realm.table().expect("table");
    assert_eq!(
        table.new_slot(Value::Integer(1), Value::Table(table.clone())),
        Err(VmError::UnsupportedTableElement)
    );
    assert_eq!(
        table.new_slot(Value::Table(table.clone()), Value::Integer(1)),
        Err(VmError::UnsupportedTableElement)
    );
    assert_eq!(
        table.new_slot(Value::Null, Value::Table(table.clone())),
        Err(VmError::NullIndex)
    );
    assert_eq!(
        table.set_existing(Value::Integer(2), Value::Table(table.clone())),
        Err(VmError::MissingIndex)
    );
    assert!(table.is_empty());
    let foreign = Realm::new().empty_root();
    assert_eq!(
        foreign.new_slot(b"table", Value::Table(table)),
        Err(VmError::RealmMismatch)
    );
}

#[test]
fn malformed_newtable_does_not_allocate_or_write_reserved_registers() {
    use ottd_script::{Instruction, Program, VmError};
    for (arg0, arg1, arg2, arg3) in [
        (0, 0, 0, 0),
        (2, 0, 0, 0),
        (1, -1, 0, 0),
        (1, 0, 1, 0),
        (1, 0, 0, 1),
    ] {
        let program = Program::from_parts(
            2,
            vec![],
            vec![Instruction {
                opcode: 0x1e,
                arg0,
                arg1,
                arg2,
                arg3,
            }],
        )
        .expect("parts");
        assert_eq!(
            Vm::new(&program).expect("frame").resume(10),
            Err(VmError::InvalidBytecode)
        );
    }
}
