//! Table allocation and literal stores have native one-dispatch accounting.
use super::{Instruction, Value, Vm, VmError};
impl Vm<'_> {
    pub(super) fn new_table(&self, instruction: Instruction) -> Result<Value, VmError> {
        if instruction.arg0 == 0 || instruction.arg2 != 0 || instruction.arg3 != 0 {
            return Err(VmError::InvalidBytecode);
        }
        self.registers
            .get(
                self.base
                    .checked_add(usize::from(instruction.arg0))
                    .ok_or(VmError::InvalidBytecode)?,
            )
            .ok_or(VmError::InvalidBytecode)?;
        let capacity = usize::try_from(instruction.arg1).map_err(|_| VmError::InvalidBytecode)?;
        crate::Table::reserved(&self.runner.get().root.realm, capacity).map(Value::Table)
    }
}
