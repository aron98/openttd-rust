use crate::{Instruction, ProgramData, Realm, Value, VmError};
use std::rc::Rc;
/// Compiled main function retaining its shared string realm.
#[derive(Clone, Debug)]
pub struct Program {
    pub(crate) data: Rc<ProgramData>,
}
impl Program {
    /// Construct bytecode in a fresh realm, reinterning all string literals.
    ///
    /// # Errors
    /// Rejects register counts outside the native range; operands are checked by execution.
    pub fn from_parts(
        stack_size: u16,
        literals: Vec<Value>,
        instructions: Vec<Instruction>,
    ) -> Result<Self, VmError> {
        if !(1..=255).contains(&stack_size) {
            return Err(VmError::InvalidBytecode);
        }
        let realm = Realm::new();
        let literals = literals
            .into_iter()
            .map(|value| match value {
                Value::String(bytes) => Ok(Value::String(realm.string(bytes.as_bytes()))),
                Value::Array(_) => Err(VmError::InvalidBytecode),
                Value::Null | Value::Integer(_) | Value::Float(_) | Value::Bool(_) => Ok(value),
            })
            .collect::<Result<Vec<_>, VmError>>()?;
        Ok(Self::freeze(ProgramData {
            stack_size,
            literals,
            instructions,
            realm,
        }))
    }
    pub(crate) fn freeze(data: ProgramData) -> Self {
        Self {
            data: Rc::new(data),
        }
    }
    /// Required register count including the reserved root slot.
    pub fn stack_size(&self) -> u16 {
        self.data.stack_size
    }
    /// Literal values in native compiler encounter order.
    pub fn literals(&self) -> &[Value] {
        &self.data.literals
    }
    /// Native instruction tuples.
    pub fn instructions(&self) -> &[Instruction] {
        &self.data.instructions
    }
}
impl PartialEq for Program {
    fn eq(&self, other: &Self) -> bool {
        self.data.stack_size == other.data.stack_size
            && self.data.literals == other.data.literals
            && self.data.instructions == other.data.instructions
    }
}
impl Eq for Program {}
