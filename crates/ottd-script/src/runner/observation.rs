//! Borrow persistent payloads without enrollment, materialization, or retaining values.
use crate::{
    Runner, Temporary,
    heap::observation::{ValueKind, ValueSnapshot},
};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RunnerSnapshot {
    pub(super) root: ValueSnapshot,
    pub(super) error: ValueSnapshot,
    pub(super) temporary: ValueSnapshot,
}
impl Runner {
    pub(super) fn snapshot(&self) -> RunnerSnapshot {
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
