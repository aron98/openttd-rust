//! Immutable, non-GC function data. No prototype owns a realm or graph handle.
use crate::{ByteString, Instruction, Value, table::scalar::Scalar};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Capture {
    Local(u8),
    Outer(u8),
    Symbol(ByteString),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Data {
    pub(crate) source_name: Option<ByteString>,
    pub(crate) name: Option<ByteString>,
    pub(crate) parameter_names: Vec<ByteString>,
    pub(crate) capture_names: Vec<ByteString>,
    pub(crate) local_names: Vec<ByteString>,
    pub(crate) stack_size: u16,
    pub(crate) literals: Vec<Scalar>,
    pub(crate) instructions: Vec<Instruction>,
    pub(crate) children: Vec<Rc<Self>>,
    pub(crate) captures: Vec<Capture>,
    pub(crate) defaults: Vec<u8>,
    pub(crate) parameters: usize,
}

/// An independently owning view of immutable function metadata.
/// Holding this view retains the prototype, not its closure or realm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prototype(pub(crate) Rc<Data>);
impl Prototype {
    /// Host-provided source name, absent for manually constructed bytecode.
    pub fn source_name(&self) -> Option<&ByteString> {
        self.0.source_name.as_ref()
    }
    /// Source-declared name; anonymous functions have no name.
    pub fn name(&self) -> Option<&ByteString> {
        self.0.name.as_ref()
    }
    /// Parameter names, including the receiver, in declaration order.
    pub fn parameter_names(&self) -> &[ByteString] {
        &self.0.parameter_names
    }
    /// Explicit capture names in declaration order.
    pub fn capture_names(&self) -> &[ByteString] {
        &self.0.capture_names
    }
    /// Local names in the native scope-release order.
    pub fn local_names(&self) -> &[ByteString] {
        &self.0.local_names
    }

    /// Child prototypes in native compiler encounter order.
    pub fn children(&self) -> Vec<Self> {
        self.0.children.iter().cloned().map(Self).collect()
    }
    /// Fixed parameter count, including the receiver.
    pub fn parameter_count(&self) -> usize {
        self.0.parameters
    }
    /// Default values' source registers in the enclosing function.
    pub fn default_registers(&self) -> &[u8] {
        &self.0.defaults
    }
    /// Capture descriptors in source order, with native kind and source operands.
    pub fn captures(&self) -> Vec<(u8, Value)> {
        self.0
            .captures
            .iter()
            .map(|capture| match capture {
                Capture::Local(index) => (0, Value::Integer(i64::from(*index))),
                Capture::Outer(index) => (2, Value::Integer(i64::from(*index))),
                Capture::Symbol(name) => (1, Value::String(name.clone())),
            })
            .collect()
    }
    /// Whether two metadata views refer to the same prototype allocation.
    pub fn same_identity(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    /// Required stack slots including the receiver.
    pub fn stack_size(&self) -> u16 {
        self.0.stack_size
    }
    /// Native instruction tuples in emission order.
    pub fn instructions(&self) -> &[Instruction] {
        &self.0.instructions
    }
    /// Independently retained scalar literals in encounter order.
    pub fn literals(&self) -> Vec<Value> {
        self.0.literals.iter().map(Scalar::value).collect()
    }
}
