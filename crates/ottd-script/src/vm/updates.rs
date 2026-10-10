//! Update stores follow native `LOCAL_INC/PLOCAL_INC` sequencing, including aliases.
use super::{Instruction, Value, Vm, VmError};
impl Vm<'_> {
    pub(super) fn update(&mut self, instruction: Instruction) -> Result<(), VmError> {
        let source = self.register(instruction.arg1)?;
        let increment = if instruction.opcode == 0x23 {
            self.register(i32::from(instruction.arg2))?
        } else {
            Value::Integer(i64::from(i8::from_ne_bytes([instruction.arg3])))
        };
        let operation = if instruction.opcode == 0x23 {
            instruction.arg3
        } else {
            b'+'
        };
        let result = source.arithmetic(&increment, operation, &self.runner.get().root.realm)?;
        let register = u8::try_from(instruction.arg1).map_err(|_| VmError::InvalidBytecode)?;
        self.write(
            instruction.arg0,
            if instruction.opcode == 0x27 {
                source
            } else {
                result.clone()
            },
        )?;
        self.write(register, result)
    }
}
