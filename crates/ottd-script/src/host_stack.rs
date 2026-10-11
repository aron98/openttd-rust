//! Explicit host-held compiled main closures, distinct from bytecode registers.
use crate::Program;
use std::{cell::RefCell, rc::Rc};
/// Host stack prefix retaining actual compiled main closures and their prototypes.
/// The active frame origin is the number of entries when the call begins.
#[derive(Clone, Debug, Default)]
pub struct HostStack(Rc<RefCell<Vec<Program>>>);
impl HostStack {
    /// Construct an empty host stack prefix.
    pub fn new() -> Self {
        Self::default()
    }
    /// Retain a compiled main closure as one real host stack entry.
    pub fn push_main(&self, program: &Program) {
        self.0.borrow_mut().push(program.clone());
    }
    /// Release all retained host entries; an active frame keeps its original origin.
    pub fn clear(&self) {
        self.0.borrow_mut().clear();
    }
    /// Number of retained host stack entries.
    pub fn len(&self) -> usize {
        self.0.borrow().len()
    }
    /// Whether the host prefix is empty.
    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_empty()
    }
    pub(crate) fn same_realm(&self, realm: &crate::Realm) -> bool {
        self.0
            .borrow()
            .iter()
            .all(|program| program.data.realm.same(realm))
    }
}
