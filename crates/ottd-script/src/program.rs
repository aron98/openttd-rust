use crate::{Instruction, Realm, Value, VmError};
/// Compiled main function retaining its shared string realm.
#[derive(Clone, Debug)]
pub struct Program {
    pub(crate) stack_size: u16,
    pub(crate) literals: Vec<Value>,
    pub(crate) instructions: Vec<Instruction>,
    pub(crate) realm: Realm,
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
                Value::String(bytes) => Value::String(realm.string(bytes.as_bytes())),
                Value::Null | Value::Integer(_) | Value::Float(_) | Value::Bool(_) => value,
            })
            .collect();
        Ok(Self {
            stack_size,
            literals,
            instructions,
            realm,
        })
    }
    /// Required register count including the reserved root slot.
    pub const fn stack_size(&self) -> u16 {
        self.stack_size
    }
    /// Literal values in native compiler encounter order.
    pub fn literals(&self) -> &[Value] {
        &self.literals
    }
    /// Native instruction tuples.
    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }
}
impl PartialEq for Program {
    fn eq(&self, other: &Self) -> bool {
        self.stack_size == other.stack_size
            && self.literals == other.literals
            && self.instructions == other.instructions
    }
}
impl Eq for Program {}
