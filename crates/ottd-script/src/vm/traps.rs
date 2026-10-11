//! Numeric trap records restore frames without acquiring additional graph owners.
use super::{Instruction, Value, Vm, VmError};

#[derive(Clone, Copy, Debug)]
pub(super) struct Trap {
    frame: usize,
    base: usize,
    top: usize,
    ip: usize,
    target: u8,
}
impl Vm<'_> {
    pub(super) const fn catchable(&self, error: VmError) -> bool {
        self.raised
            && match error {
                VmError::ThrownValue
                | VmError::NativeCall
                | VmError::NotCallable
                | VmError::NullIndex
                | VmError::MissingIndex
                | VmError::UnsupportedReceiver
                | VmError::OperandType
                | VmError::DivisionByZero
                | VmError::ModuloByZero
                | VmError::UnsupportedTableOrdering
                | VmError::UnsupportedArrayOrdering
                | VmError::UnsupportedRuntimeValue => true,
                VmError::UnsupportedTableHashTarget
                | VmError::UnsupportedTableElement
                | VmError::UnsupportedArrayElement
                | VmError::UnsupportedIndexConversion
                | VmError::UndefinedNativeVarargsTailCall
                | VmError::Allocation
                | VmError::RealmMismatch
                | VmError::UnsupportedOpcode(_)
                | VmError::InvalidBytecode
                | VmError::UnsupportedOverflow
                | VmError::UnsupportedShiftCount
                | VmError::BudgetOverflow
                | VmError::Finished => false,
            }
    }

    pub(super) fn has_frame_trap(&self) -> bool {
        self.traps
            .last()
            .is_some_and(|trap| trap.frame == self.frames.len())
    }
    pub(super) fn trap_instruction(&mut self, i: Instruction) -> Result<(), VmError> {
        if i.arg2 != 0 || i.arg3 != 0 {
            return Err(VmError::InvalidBytecode);
        }
        match i.opcode {
            0x38 => self.push_trap(i),
            0x39 if i.arg1 == 0 => self.pop_traps(i.arg0),
            0x3a if i.arg1 == 0 => {
                let error = self.register(i32::from(i.arg0))?;
                self.runner.get_mut().native_error = error;
                self.raised = true;
                Err(VmError::ThrownValue)
            }
            _ => Err(VmError::InvalidBytecode),
        }
    }
    fn push_trap(&mut self, i: Instruction) -> Result<(), VmError> {
        let slot = self
            .base
            .checked_add(usize::from(i.arg0))
            .ok_or(VmError::InvalidBytecode)?;
        if slot >= self.top {
            return Err(VmError::InvalidBytecode);
        }
        let ip = self
            .ip
            .checked_add_signed(isize::try_from(i.arg1).map_err(|_| VmError::InvalidBytecode)?)
            .ok_or(VmError::InvalidBytecode)?;
        if ip >= self.program()?.0.instructions.len() {
            return Err(VmError::InvalidBytecode);
        }
        self.traps.try_reserve(1).map_err(|_| VmError::Allocation)?;
        self.traps.push(Trap {
            frame: self.frames.len(),
            base: self.base,
            top: self.top,
            ip,
            target: i.arg0,
        });
        Ok(())
    }
    fn pop_traps(&mut self, count: u8) -> Result<(), VmError> {
        let start = self
            .traps
            .len()
            .checked_sub(usize::from(count))
            .ok_or(VmError::InvalidBytecode)?;
        if self
            .traps
            .get(start..)
            .ok_or(VmError::InvalidBytecode)?
            .iter()
            .any(|trap| trap.frame != self.frames.len())
        {
            return Err(VmError::InvalidBytecode);
        }
        self.traps.truncate(start);
        Ok(())
    }
    pub(super) fn restore_trap(&mut self) -> Result<bool, VmError> {
        let Some(trap) = self.traps.last().copied() else {
            return Ok(false);
        };
        let error = self.runner.get().native_error.clone();
        let last_top = self.top;
        while self.frames.len() > trap.frame {
            self.unwind_parent()?;
        }
        if self.frames.len() != trap.frame {
            return Err(VmError::InvalidBytecode);
        }
        self.ip = trap.ip;
        self.top = trap.top;
        self.base = trap.base;
        self.write(trap.target, error.clone())?;
        let _trap = self.traps.pop().ok_or(VmError::InvalidBytecode)?;
        if last_top >= self.top {
            for slot in (self.top..=last_top).rev() {
                *self.native_slot(slot)? = Value::Null;
            }
        }
        drop(error);
        Ok(true)
    }
}

#[cfg(test)]
mod owners;
