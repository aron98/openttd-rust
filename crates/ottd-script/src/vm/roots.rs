//! Root identity is checked independently of scalar operand access.
use super::{Instruction, RootEnvironment, Slot, Temporary, Value, Vm, VmError};
impl Vm<'_> {
    pub(super) fn load_nulls(&mut self, i: Instruction) -> Result<(), VmError> {
        let length = usize::try_from(i.arg1).map_err(|_| VmError::InvalidBytecode)?;
        let start = usize::from(i.arg0);
        if start == 0 {
            return Err(VmError::InvalidBytecode);
        }
        let end = start.checked_add(length).ok_or(VmError::InvalidBytecode)?;
        self.registers
            .get_mut(start..end)
            .ok_or(VmError::InvalidBytecode)?
            .fill(Slot::Scalar(Value::Null));
        Ok(())
    }
    pub(super) fn store_temporary(&mut self, value: Value) -> Result<Value, VmError> {
        self.runner.get_mut().temporary = Temporary::Scalar(value);
        match &self.runner.get().temporary {
            Temporary::Scalar(value) => Ok(value.clone()),
            Temporary::MainProgram { .. } => Err(VmError::UnsupportedRuntimeValue),
        }
    }
    fn root_receiver(&self, register: i32) -> Result<&RootEnvironment, VmError> {
        let index = usize::try_from(register).map_err(|_| VmError::InvalidBytecode)?;
        match self.registers.get(index).ok_or(VmError::InvalidBytecode)? {
            Slot::Root(root) => Ok(root),
            Slot::Scalar(_) => Err(VmError::UnsupportedReceiver),
        }
    }
    pub(super) fn write_root(
        &mut self,
        register: u8,
        root: RootEnvironment,
    ) -> Result<(), VmError> {
        if register == 0 {
            return Err(VmError::InvalidBytecode);
        }
        *self
            .registers
            .get_mut(usize::from(register))
            .ok_or(VmError::InvalidBytecode)? = Slot::Root(root);
        Ok(())
    }
    pub(super) fn root_get(&mut self, instruction: Instruction) -> Result<Value, VmError> {
        let (receiver, key) = if instruction.opcode == 0x09 {
            (i32::from(instruction.arg2), self.literal(instruction.arg1)?)
        } else {
            (
                instruction.arg1,
                self.register(i32::from(instruction.arg2))?,
            )
        };
        let Value::String(key) = key else {
            return Err(VmError::UnsupportedRuntimeValue);
        };
        let root = self.root_receiver(receiver)?;
        let value = root.lookup(
            key.as_bytes(),
            self.root_receiver(0)?,
            &self.runner.get().root,
        )?;
        self.store_temporary(value)
    }
    pub(super) fn root_store(&mut self, instruction: Instruction) -> Result<(), VmError> {
        let key = self.register(i32::from(instruction.arg2))?;
        let Value::String(key) = key else {
            return Err(VmError::UnsupportedRuntimeValue);
        };
        let value = self.register(i32::from(instruction.arg3))?;
        let root = self.root_receiver(instruction.arg1)?;
        if instruction.opcode == 0x0b {
            root.new_slot(key.as_bytes(), value.clone());
        } else if let Err(error) = root.set_existing(key.as_bytes(), value.clone()) {
            if root.same(self.root_receiver(0)?) {
                self.runner
                    .get()
                    .root
                    .set_existing(key.as_bytes(), value.clone())?;
            } else {
                return Err(error);
            }
        }
        if instruction.arg0 != instruction.arg3 {
            self.write(instruction.arg0, value)?;
        }
        Ok(())
    }
}
