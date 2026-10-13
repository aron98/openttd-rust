//! Borrow persistent payloads without enrollment, materialization, or retaining values.
use crate::{
    Runner, Temporary,
    heap::observation::{ValueKind, ValueSnapshot},
    heap::{Handle, Object},
    native_session::payload,
};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RunnerSnapshot {
    pub(super) root: ValueSnapshot,
    pub(super) error: ValueSnapshot,
    pub(super) temporary: ValueSnapshot,
}
impl Runner {
    #[expect(
        clippy::unreachable,
        reason = "A bound runner context owns context storage"
    )]
    pub(super) fn snapshot(&self) -> RunnerSnapshot {
        // A bound friend context snapshots its own persistent fields, exactly
        // like the native observer reading a friend VM's `_roottable`; the
        // finalized context's cleared fields read as their native Null.
        if let Some(context) = self.root.context() {
            let realm = &self.root.as_ref().realm;
            let fields = Handle::from_edge(context, realm).read(|object| match object {
                Object::Context(data) => [
                    data.persistent()[payload::ROOT].clone(),
                    data.persistent()[payload::ERROR].clone(),
                    data.persistent()[payload::TEMP].clone(),
                ],
                Object::Array(_)
                | Object::NativeClosure(_)
                | Object::ScriptClosure(_)
                | Object::Table(_) => {
                    unreachable!("a bound runner context owns context storage")
                }
            });
            return RunnerSnapshot {
                root: ValueSnapshot::stored(&fields[0]),
                error: ValueSnapshot::stored(&fields[1]),
                temporary: ValueSnapshot::stored(&fields[2]),
            };
        }
        let temporary = match self.temporary.as_ref() {
            Temporary::Value(value) => ValueSnapshot::value(value),
            Temporary::MainProgram { _program: program } => {
                ValueSnapshot::handle(ValueKind::ScriptClosure, &program.0)
            }
        };
        RunnerSnapshot {
            root: ValueSnapshot::handle(ValueKind::Table, &self.root.as_ref().table.0),
            error: ValueSnapshot::value(self.native_error.as_ref()),
            temporary,
        }
    }
}
