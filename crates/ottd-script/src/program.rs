use crate::{Instruction, ProgramData, Realm, Value, VmError};
use std::rc::Rc;
/// Compiled main function retaining its shared string realm.
#[derive(Clone, Debug)]
pub struct Program {
    pub(crate) data: crate::ScriptClosure,
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
                Value::Array(_)
                | Value::Table(_)
                | Value::NativeClosure(_)
                | Value::ScriptClosure(_) => Err(VmError::InvalidBytecode),
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
    #[expect(
        clippy::expect_used,
        reason = "Compiler freeze has scalar literals and native-style infallible object allocation"
    )]
    pub(crate) fn freeze(data: ProgramData) -> Self {
        let prototype = crate::prototype::Data {
            source_name: None,
            name: None,
            parameter_names: Vec::new(),
            capture_names: Vec::new(),
            local_names: Vec::new(),
            stack_size: data.stack_size,
            literals: data
                .literals
                .into_iter()
                .map(|value| {
                    crate::table::scalar::Scalar::from_value(value, &data.realm)
                        .expect("scalar prototype literal")
                })
                .collect(),
            instructions: data.instructions,
            children: Vec::new(),
            captures: Vec::new(),
            defaults: Vec::new(),
            parameters: 1,
            variadic: false,
        };
        Self {
            data: crate::ScriptClosure::new(&data.realm, Rc::new(prototype))
                .expect("main closure allocation"),
        }
    }
    /// Retain immutable metadata independently of this closure.
    pub fn prototype(&self) -> crate::Prototype {
        self.data.prototype()
    }

    /// Required register count including the reserved root slot.
    pub fn stack_size(&self) -> u16 {
        self.data.read(|data| data.prototype.stack_size)
    }
    /// Literal values in native compiler encounter order.
    pub fn literals(&self) -> Vec<Value> {
        self.data.prototype().literals()
    }
    /// Native instruction tuples.
    pub fn instructions(&self) -> Vec<Instruction> {
        self.data.read(|data| data.prototype.instructions.clone())
    }
}
impl PartialEq for Program {
    fn eq(&self, other: &Self) -> bool {
        self.prototype() == other.prototype()
    }
}
impl Eq for Program {}
