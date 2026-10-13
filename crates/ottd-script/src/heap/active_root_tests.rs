//! Actual print-callback collection retains the current admitted VM roots.
use super::ObjectId;
use crate::{Execution, PrintSink, Realm, Runner, Value};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
struct Collection {
    count: usize,
    marked: Vec<char>,
}
#[derive(Debug)]
struct Collect {
    realm: Realm,
    identities: [(char, ObjectId); 2],
    observations: Rc<RefCell<Vec<Collection>>>,
}
impl PrintSink for Collect {
    fn print(&mut self, bytes: &[u8]) {
        assert_eq!(bytes, b"collect-active");
        let count = self.realm.heap.collect();
        let marked = self
            .realm
            .heap
            .chain
            .borrow()
            .iter()
            .filter_map(|id| {
                self.identities
                    .iter()
                    .find_map(|(label, known)| (id == known).then_some(*label))
            })
            .collect();
        self.observations
            .borrow_mut()
            .push(Collection { count, marked });
    }
}
struct Case {
    name: &'static str,
    source: &'static str,
    native: &'static str,
}
const CASES: [Case; 5] = [
    Case {
        name: "frame",
        source: include_str!(
            "../../../../scripts/compat/script-vm/tables/fixtures/active/frame.nut"
        ),
        native: include_str!("../../tests/active-owners/frame-on.txt"),
    },
    Case {
        name: "varargs",
        source: include_str!(
            "../../../../scripts/compat/script-vm/tables/fixtures/active/varargs.nut"
        ),
        native: include_str!("../../tests/active-owners/varargs-on.txt"),
    },
    Case {
        name: "error",
        source: include_str!(
            "../../../../scripts/compat/script-vm/tables/fixtures/active/error.nut"
        ),
        native: include_str!("../../tests/active-owners/error-on.txt"),
    },
    Case {
        name: "closure",
        source: include_str!(
            "../../../../scripts/compat/script-vm/tables/fixtures/active/closure.nut"
        ),
        native: include_str!("../../tests/active-owners/closure-on.txt"),
    },
    Case {
        name: "allocated-receiver",
        source: include_str!(
            "../../../../scripts/compat/script-vm/tables/fixtures/active/allocated-receiver.nut"
        ),
        native: include_str!("../../tests/active-owners/allocated-receiver-on.txt"),
    },
];
fn original(case: &Case) -> String {
    std::env::var("OTTD_SCRIPT_ACTIVE_OWNER_DIR").map_or_else(
        |_| case.native.to_owned(),
        |directory| {
            std::fs::read_to_string(
                std::path::Path::new(&directory).join(format!("{}-on-on.stdout", case.name)),
            )
            .expect("original active root trace")
        },
    )
}
fn check(case: &Case) {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let program = runner.compile(case.source).expect("defined source");
    let delegate = realm.table().expect("delegate");
    let table = realm.table().expect("table");
    table.set_delegate(Some(&delegate)).expect("delegate edge");
    table
        .new_slot(Value::Integer(0), Value::Table(table.clone()))
        .expect("self");
    let identities = [('O', table.0.edge.life.id), ('D', delegate.0.edge.life.id)];
    runner
        .root
        .as_ref()
        .new_slot(
            b"__owner_setup",
            Value::Array(
                realm
                    .array(vec![Value::Table(table), Value::Table(delegate)])
                    .expect("setup"),
            ),
        )
        .expect("actual configured root");
    let observations = Rc::new(RefCell::new(Vec::new()));
    runner.set_print_sink(Some(Box::new(Collect {
        realm,
        identities,
        observations: Rc::clone(&observations),
    })));
    let mut vm = runner.start(&program).expect("frame");
    assert_eq!(
        vm.resume(10000),
        Ok(Execution::Returned(Value::Integer(1))),
        "{}",
        case.name
    );
    let native = original(case);
    let count = native
        .lines()
        .find_map(|line| line.strip_prefix("collected active "))
        .expect("original collection")
        .parse::<usize>()
        .expect("count");
    let expected = native
        .lines()
        .filter_map(|line| line.strip_prefix("chain"))
        .nth(1)
        .expect("original marked chain")
        .split_whitespace()
        .map(|label| label.chars().next().expect("label"))
        .collect::<Vec<_>>();
    let observations = observations.borrow();
    assert_eq!(observations.len(), 1);
    let observed = observations.first().expect("one callback");
    assert_eq!(observed.count, count, "{}", case.name);
    assert_eq!(observed.marked, expected, "{}", case.name);
    let remaining = native
        .lines()
        .find_map(|line| line.strip_prefix("completed 1 0 "))
        .expect("successful original frame")
        .split_whitespace()
        .next()
        .expect("debt")
        .parse::<i64>()
        .expect("debt");
    assert_eq!(vm.remaining_ops(), remaining, "{}", case.name);
}
#[test]
fn original_active_frame_capture_error_varargs_and_allocated_stack_roots_survive() {
    for case in &CASES {
        check(case);
    }
}
