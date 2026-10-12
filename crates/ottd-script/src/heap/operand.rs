//! An instruction-local reference owner is not a collector root.
use crate::{Value, VmError};

#[derive(Debug)]
pub(crate) struct Operand(Value);
impl Operand {
    pub(crate) fn new(mut value: Value) -> Result<Self, VmError> {
        match &mut value {
            Value::Table(table) => table.0.edge.make_reference()?,
            Value::Array(array) => array.0.edge.make_reference()?,
            Value::NativeClosure(closure) => closure.0.edge.make_reference()?,
            Value::ScriptClosure(closure) => closure.0.edge.make_reference()?,
            Value::Null
            | Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::String(_) => {}
        }
        Ok(Self(value))
    }
    pub(crate) const fn value(&self) -> &Value {
        &self.0
    }
}
