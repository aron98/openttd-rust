//! Native finalizers retain reference-owned array and native-closure allocations.
use super::{Object, ObjectId, Operand};
use crate::{Execution, PrintSink, Realm, Runner, Value};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Copy)]
enum Kind {
    Array,
    AcyclicArray,
    Native,
}
#[derive(Debug, PartialEq, Eq)]
struct State {
    references: usize,
    elements: usize,
}
#[derive(Debug, Default)]
struct Trace {
    states: Vec<Option<State>>,
    collected: Vec<usize>,
}
#[derive(Debug)]
struct Sink {
    realm: Realm,
    identity: ObjectId,
    trace: Rc<RefCell<Trace>>,
}
impl Sink {
    fn snapshot(&self) -> Option<State> {
        let entries = self.realm.heap.entries.borrow();
        let entry = entries.get(self.identity.index)?;
        if entry.generation != self.identity.generation {
            return None;
        }
        let elements = match entry.object.as_deref()? {
            Object::Array(values) => values.len(),
            Object::NativeClosure(data) => data.values().count(),
            Object::Table(_) | Object::ScriptClosure(_) | Object::Context(_) => return None,
        };
        Some(State {
            references: entry.life.as_ref()?.references.get(),
            elements,
        })
    }
}
impl PrintSink for Sink {
    fn print(&mut self, bytes: &[u8]) {
        assert_eq!(bytes, b"collect-active");
        let before = self.snapshot();
        let count = self.realm.heap.collect();
        let after = self.snapshot();
        let mut trace = self.trace.borrow_mut();
        trace.states.extend([before, after]);
        trace.collected.push(count);
    }
}
fn native(kind: Kind) -> String {
    let (name, frozen) = match kind {
        Kind::Array => (
            "local-array",
            include_str!("../../tests/active-owners/local-array-on.txt"),
        ),
        Kind::AcyclicArray => (
            "local-array-acyclic",
            include_str!("../../tests/active-owners/local-array-acyclic-on.txt"),
        ),
        Kind::Native => (
            "local-native",
            include_str!("../../tests/active-owners/local-native-on.txt"),
        ),
    };
    std::env::var("OTTD_SCRIPT_ACTIVE_OWNER_DIR").map_or_else(
        |_| frozen.to_owned(),
        |directory| {
            std::fs::read_to_string(
                std::path::Path::new(&directory).join(format!("{name}-on-on.stdout")),
            )
            .expect("actual native reference trace")
        },
    )
}
fn expected(kind: Kind, native: &str) -> Vec<Option<State>> {
    let prefix = match kind {
        Kind::Array | Kind::AcyclicArray => "array-reference ",
        Kind::Native => "native-reference ",
    };
    native
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .map(|line| {
            let mut fields = line.split_whitespace();
            let references = fields
                .next()
                .expect("reference count")
                .parse()
                .expect("count");
            if matches!(kind, Kind::Native) {
                assert_eq!(fields.next(), Some("-2"));
            }
            let elements = fields
                .next()
                .expect("element count")
                .parse()
                .expect("count");
            if matches!(kind, Kind::Native) {
                assert_eq!(fields.next(), Some("array"));
            }
            assert_eq!(fields.next(), None);
            Some(State {
                references,
                elements,
            })
        })
        .collect()
}
fn run(kind: Kind) {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let program = runner
        .compile(include_str!(
            "../../../../scripts/compat/script-vm/tables/fixtures/active/local.nut"
        ))
        .expect("marker source");
    let delegate = realm.table().expect("delegate");
    let table = realm.table().expect("table");
    table.set_delegate(Some(&delegate)).expect("delegate edge");
    table
        .new_slot(Value::Integer(0), Value::Table(table.clone()))
        .expect("self");
    let identities = [('O', table.0.edge.life.id), ('D', delegate.0.edge.life.id)];
    *realm.heap.retired.borrow_mut() = Some(Vec::with_capacity(64));
    let value = match kind {
        Kind::Array => Value::Array(
            realm
                .array(vec![Value::Table(table), Value::Table(delegate)])
                .expect("array"),
        ),
        Kind::AcyclicArray => {
            drop(
                table
                    .delete_slot(Value::Integer(0))
                    .expect("remove self edge"),
            );
            table.set_delegate(None).expect("detach delegate");
            Value::Array(
                realm
                    .array(vec![Value::Table(table), Value::Table(delegate)])
                    .expect("array"),
            )
        }
        Kind::Native => {
            drop(table);
            drop(delegate);
            runner
                .root
                .as_ref()
                .delete_slot(b"array")
                .expect("native owner")
        }
    };
    let identity = match &value {
        Value::Array(array) => array.0.edge.life.id,
        Value::NativeClosure(closure) => closure.0.edge.life.id,
        Value::Table(_)
        | Value::ScriptClosure(_)
        | Value::Null
        | Value::Integer(_)
        | Value::Float(_)
        | Value::Bool(_)
        | Value::String(_) => panic!("expected array or native closure"),
    };
    let retained = Operand::new(value).expect("reference owner");
    let trace = Rc::new(RefCell::new(Trace::default()));
    runner.set_print_sink(Some(Box::new(Sink {
        realm: realm.clone(),
        identity,
        trace: Rc::clone(&trace),
    })));
    let mut vm = runner.start(&program).expect("frame");
    assert_eq!(vm.resume(10000), Ok(Execution::Returned(Value::Integer(1))));
    let native = native(kind);
    let collected = native
        .lines()
        .find_map(|line| line.strip_prefix("collected active "))
        .expect("original active count")
        .parse::<usize>()
        .expect("count");
    assert_eq!(trace.borrow().collected, [collected]);
    let retired = realm
        .heap
        .retired
        .borrow()
        .as_ref()
        .expect("recorder")
        .iter()
        .filter_map(|id| {
            identities
                .iter()
                .find_map(|(label, known)| (id == known).then_some(*label))
        })
        .collect::<Vec<_>>();
    let expected_retirements = native
        .lines()
        .filter_map(|line| line.strip_prefix("free "))
        .filter_map(|event| match event {
            "O 88" => Some('O'),
            "D 88" => Some('D'),
            "A 72" | "N 144" => None,
            unexpected => panic!("unknown native free event {unexpected}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(retired, expected_retirements);
    assert_eq!(trace.borrow().states, expected(kind, &native));
    drop(retained);
    // End the recorder before the session's own close-order retirements,
    // which happen after this observation window exactly like the native
    // recorder detaching before the shared state's teardown.
    *realm.heap.retired.borrow_mut() = None;
}
#[test]
fn reference_owned_array_remains_accessible_after_active_finalization() {
    run(Kind::Array);
}
#[test]
fn reference_owned_native_closure_retains_metadata_after_active_finalization() {
    run(Kind::Native);
}

#[test]
fn reference_owned_array_finalization_releases_children_in_native_order() {
    run(Kind::AcyclicArray);
}
