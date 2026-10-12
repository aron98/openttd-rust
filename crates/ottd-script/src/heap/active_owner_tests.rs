//! Original host collection distinguishes reference ownership from collector roots.
use super::{Heap, Object, ObjectId, Operand};
use crate::{Execution, PrintSink, Realm, Runner, Value};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    references: usize,
    used: usize,
    delegate: bool,
}
fn snapshot(heap: &Heap, id: ObjectId) -> Option<Snapshot> {
    let entries = heap.entries.borrow();
    let entry = entries.get(id.index)?;
    if entry.generation != id.generation {
        return None;
    }
    let life = entry.life.as_ref()?;
    match entry.object.as_ref()? {
        Object::Table(data) => Some(Snapshot {
            references: life.references.get(),
            used: data.nodes.len(),
            delegate: data.delegate.is_some(),
        }),
        Object::Array(_) | Object::NativeClosure(_) | Object::ScriptClosure(_) => None,
    }
}
#[derive(Debug, Default)]
struct Observations {
    states: Vec<Option<Snapshot>>,
    collected: Vec<usize>,
}
#[derive(Debug)]
struct CollectAtPrint {
    realm: Realm,
    identity: ObjectId,
    observations: Rc<RefCell<Observations>>,
}
impl PrintSink for CollectAtPrint {
    fn print(&mut self, bytes: &[u8]) {
        assert_eq!(bytes, b"collect-active");
        let before = snapshot(&self.realm.heap, self.identity);
        let collected = self.realm.heap.collect();
        let after = snapshot(&self.realm.heap, self.identity);
        let mut observations = self.observations.borrow_mut();
        observations.states.extend([before, after]);
        observations.collected.push(collected);
    }
}
fn native() -> String {
    std::env::var("OTTD_SCRIPT_ACTIVE_OWNER_DIR").map_or_else(
        |_| include_str!("../../tests/active-owners/local-on.txt").to_owned(),
        |directory| {
            std::fs::read_to_string(std::path::Path::new(&directory).join("local-on-on.stdout"))
                .expect("actual original local-reference trace")
        },
    )
}
fn expected_snapshots(native: &str) -> Vec<Option<Snapshot>> {
    native
        .lines()
        .filter_map(|line| line.strip_prefix("local-reference "))
        .map(|line| {
            let mut fields = line.split_whitespace();
            let state = Snapshot {
                references: fields
                    .next()
                    .expect("reference count")
                    .parse()
                    .expect("count"),
                used: fields.next().expect("CountUsed").parse().expect("count"),
                delegate: fields.next().expect("delegate") == "D",
            };
            assert_eq!(fields.next(), None);
            Some(state)
        })
        .collect()
}
#[test]
fn reference_owned_table_keeps_finalized_storage_during_active_collection() {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let program = runner
        .compile(include_str!(
            "../../../../scripts/compat/script-vm/tables/fixtures/active/local.nut"
        ))
        .expect("finite marker program");
    let delegate = realm.table().expect("delegate");
    let table = realm.table().expect("table");
    table.set_delegate(Some(&delegate)).expect("delegate edge");
    table
        .new_slot(Value::Integer(0), Value::Table(table.clone()))
        .expect("self edge");
    let identity = table.0.edge.life.id;
    let retained = Operand::new(Value::Table(table)).expect("reference owner");
    drop(delegate);
    let observations = Rc::new(RefCell::new(Observations::default()));
    runner.set_print_sink(Some(Box::new(CollectAtPrint {
        realm: realm.clone(),
        identity,
        observations: Rc::clone(&observations),
    })));
    let mut vm = runner.start(&program).expect("frame");
    assert_eq!(vm.resume(10000), Ok(Execution::Returned(Value::Integer(1))));
    assert_eq!(vm.remaining_ops(), 9994);
    assert_eq!(observations.borrow().collected, [1]);
    assert_eq!(observations.borrow().states, expected_snapshots(&native()));
    let Value::Table(table) = retained.value() else {
        panic!("table operand");
    };
    assert_eq!(table.raw_get(Value::Integer(0)), Ok(None));
    let clone = table.shallow_clone().expect("clone finalized table");
    let original = native();
    let clone_count = original
        .lines()
        .find_map(|line| line.strip_prefix("clone-used "))
        .expect("native clone continuation")
        .split_whitespace()
        .next()
        .expect("count")
        .parse::<usize>()
        .expect("count");
    assert_eq!(clone.len(), clone_count);
    drop(clone);
    table
        .new_slot(Value::Integer(1), Value::Integer(9))
        .expect("defined insertion");
    let inserted_count = original
        .lines()
        .find_map(|line| line.strip_prefix("insert-used "))
        .expect("native insertion continuation")
        .split_whitespace()
        .next()
        .expect("count")
        .parse::<usize>()
        .expect("count");
    assert_eq!(table.len(), inserted_count);
    assert_eq!(
        table.raw_get(Value::Integer(1)),
        Ok(Some(Value::Integer(9)))
    );
    drop(retained);
    assert_eq!(
        snapshot(&realm.heap, identity),
        None,
        "released reference retires immediately"
    );
}
