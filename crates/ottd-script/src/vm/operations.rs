//! Reached native arithmetic/comparison failures raise objects at their opcode boundary.
use super::{Instruction, Value, Vm, VmError};

impl Vm<'_> {
    pub(super) fn binary_operation(&mut self, i: Instruction) -> Result<Value, VmError> {
        let left = self.register(i32::from(i.arg2))?;
        let right = self.register(i.arg1)?;
        match i.opcode {
            0x12 => self.bitwise_values(left, right, i.arg3),
            0x28 => self.compare_values(&left, &right, i.arg3),
            _ => Err(VmError::InvalidBytecode),
        }
    }

    pub(super) fn arithmetic_values(
        &mut self,
        left: &Value,
        right: &Value,
        operation: u8,
    ) -> Result<Value, VmError> {
        if !matches!(operation, b'+' | b'-' | b'*' | b'/' | b'%') {
            return Err(VmError::InvalidBytecode);
        }
        let result = left.arithmetic(right, operation, &self.runner.get().root.realm);
        result.map_err(|error| {
            let message = match error {
                VmError::DivisionByZero => b"division by zero".to_vec(),
                VmError::ModuloByZero => b"modulo by zero".to_vec(),
                VmError::OperandType => format!(
                    "arith op {} on between '{}' and '{}'",
                    char::from(operation),
                    left.type_label(),
                    right.type_label()
                )
                .into_bytes(),
                other => return other,
            };
            self.native_failure(&message, error)
        })
    }
    pub(super) fn bitwise_values(
        &mut self,
        left: Value,
        right: Value,
        operation: u8,
    ) -> Result<Value, VmError> {
        let left_type = left.type_label();
        let right_type = right.type_label();
        left.bitwise(right, operation).map_err(|error| {
            if error != VmError::OperandType {
                return error;
            }
            let message = format!("bitwise op between '{left_type}' and '{right_type}'");
            self.native_failure(message.as_bytes(), error)
        })
    }
    pub(super) fn negate_value(&mut self, value: &Value) -> Result<Value, VmError> {
        value.negate().map_err(|error| {
            if error != VmError::OperandType {
                return error;
            }
            let message = format!("attempt to negate a {}", value.type_label());
            self.native_failure(message.as_bytes(), error)
        })
    }
    pub(super) fn compare_values(
        &mut self,
        left: &Value,
        right: &Value,
        selector: u8,
    ) -> Result<Value, VmError> {
        if !matches!(selector, 0 | 2 | 3 | 4) {
            return Err(VmError::InvalidBytecode);
        }
        left.compare(right, selector).map_err(|error| {
            if !native_comparison_error(left, right) {
                return error;
            }
            let operands = super::diagnostics::diagnostic_prefix(left).and_then(|left| {
                super::diagnostics::diagnostic_prefix(right).map(|right| (left, right))
            });
            let (left, right) = match operands {
                Ok(operands) => operands,
                Err(failure) => return failure,
            };
            let message = [
                b"comparison between '".as_slice(),
                &left,
                b"' and '",
                &right,
                b"'",
            ]
            .concat();
            self.native_failure(&message, error)
        })
    }
}

fn native_comparison_error(left: &Value, right: &Value) -> bool {
    !(std::mem::discriminant(left) == std::mem::discriminant(right)
        || matches!(left, Value::Null)
        || matches!(right, Value::Null)
        || (matches!(left, Value::Integer(_) | Value::Float(_))
            && matches!(right, Value::Integer(_) | Value::Float(_))))
}
