//! A public Value clone acquires a host root even from a private reference view.
use super::Operand;
use crate::{Execution, PrintSink, Realm, Runner, Value};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
struct Collect {
    realm: Realm,
    counts: Rc<RefCell<Vec<usize>>>,
}
impl PrintSink for Collect {
    fn print(&mut self, bytes: &[u8]) {
        assert_eq!(bytes, b"collect-active");
        let count = self.realm.heap.collect();
        self.counts.borrow_mut().push(count);
    }
}
#[test]
fn public_value_clone_keeps_its_host_root_contract_for_private_operand_views() {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let program = runner
        .compile(include_str!(
            "../../../../scripts/compat/script-vm/tables/fixtures/active/local.nut"
        ))
        .expect("finite source");
    let delegate = realm.table().expect("delegate");
    let table = realm.table().expect("table");
    table.set_delegate(Some(&delegate)).expect("delegate");
    table
        .new_slot(Value::Integer(0), Value::Table(table.clone()))
        .expect("self");
    let operand = Operand::new(Value::Table(table)).expect("local reference");
    drop(delegate);
    let public_owner = operand.value().clone();
    let counts = Rc::new(RefCell::new(Vec::new()));
    runner.set_print_sink(Some(Box::new(Collect {
        realm: realm.clone(),
        counts: Rc::clone(&counts),
    })));
    let mut vm = runner.start(&program).expect("frame");
    assert_eq!(vm.resume(10000), Ok(Execution::Returned(Value::Integer(1))));
    let native = std::env::var("OTTD_SCRIPT_ACTIVE_OWNER_DIR").map_or_else(
        |_| include_str!("../../tests/active-owners/local-host-on.txt").to_owned(),
        |directory| {
            std::fs::read_to_string(
                std::path::Path::new(&directory).join("local-host-on-on.stdout"),
            )
            .expect("native dual-owner trace")
        },
    );
    let expected = native
        .lines()
        .find_map(|line| line.strip_prefix("collected active "))
        .expect("collection count")
        .parse::<usize>()
        .expect("count");
    assert_eq!(*counts.borrow(), [expected]);
    let Value::Table(table) = &public_owner else {
        panic!("table clone");
    };
    assert_eq!(
        table.raw_get(Value::Integer(0)),
        Ok(Some(public_owner.clone()))
    );
    drop(public_owner);
    drop(operand);
    let released = native
        .lines()
        .find_map(|line| line.strip_prefix("collected released "))
        .expect("native released count")
        .parse::<usize>()
        .expect("count");
    assert_eq!(realm.heap.collect(), released);
}
