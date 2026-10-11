use crate::{
    Instruction, Program, ProgramData, RootEnvironment, Runner, RunnerFailure, Storage, Temporary,
    Value, VmError,
};
use std::rc::Rc;
#[expect(
    clippy::redundant_pub_crate,
    reason = "Native numeric conversion is shared with table hashing"
)]
pub(crate) mod arrays;
mod dispatch;
mod foreach;
mod roots;
mod tables;
mod updates;
#[derive(Clone, Debug)]
enum Slot {
    Value(Value),
    Root(RootEnvironment),
}
impl Slot {
    const fn value(&self) -> Result<&Value, VmError> {
        match self {
            Self::Value(value) => Ok(value),
            Self::Root(_) => Err(VmError::UnsupportedRuntimeValue),
        }
    }
}

/// Outcome of an operation-budget slice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Execution {
    /// Budget exhausted before fetching the next instruction.
    Suspended,
    /// Main function returned an independently owned value.
    Returned(Value),
}
/// Register VM retaining instruction pointer, registers and operation debt.
#[derive(Debug)]
pub struct Vm<'a> {
    program: Option<Rc<ProgramData>>,
    registers: Vec<Slot>,
    runner: Storage<'a>,
    started: bool,
    ip: usize,
    remaining: i64,
    finished: bool,
}
impl<'a> Vm<'a> {
    /// Create a fresh main frame. Budget is supplied through `resume` credits.
    ///
    /// # Errors
    /// Rejects stack sizes outside the native register range.
    pub fn new(program: &'a Program) -> Result<Self, VmError> {
        Self::with_runner(
            program,
            Storage::Owned(Runner::new(program.data.realm.empty_root())),
        )
    }
    pub(crate) fn with_runner(program: &Program, runner: Storage<'a>) -> Result<Self, VmError> {
        if !(1..=255).contains(&program.data.stack_size) {
            return Err(VmError::InvalidBytecode);
        }
        let mut registers = vec![Slot::Value(Value::Null); usize::from(program.data.stack_size)];
        *registers.first_mut().ok_or(VmError::InvalidBytecode)? =
            Slot::Root(runner.get().root.clone());
        Ok(Self {
            program: Some(Rc::clone(&program.data)),
            registers,
            runner,
            started: false,
            ip: 0,
            remaining: 0,
            finished: false,
        })
    }
    fn program(&self) -> Result<&ProgramData, VmError> {
        self.program.as_deref().ok_or(VmError::Finished)
    }
    /// Index of the next instruction; meaningful at suspension boundaries.
    pub const fn instruction_pointer(&self) -> usize {
        self.ip
    }
    /// Remaining operations, including pre-dispatch suspension debt.
    pub const fn remaining_ops(&self) -> i64 {
        self.remaining
    }
    /// Add operations to the retained counter and execute until suspension/return.
    ///
    /// A zero credit still charges one operation before suspension, as native does.
    ///
    /// # Errors
    /// Rejects invalid instructions, unsupported operations, operand errors, signed
    /// arithmetic overflow, budget overflow, and calls after completion/failure.
    pub fn resume(&mut self, credit: u32) -> Result<Execution, VmError> {
        if self.finished {
            return Err(VmError::Finished);
        }
        self.remaining = self
            .remaining
            .checked_add(i64::from(credit))
            .ok_or(VmError::BudgetOverflow)?;
        if !self.started {
            let program = Rc::clone(self.program.as_ref().ok_or(VmError::Finished)?);
            self.runner.get_mut().temporary = Temporary::MainProgram { _program: program };
            self.started = true;
        }
        let result = self.run();
        match result {
            Ok(Execution::Suspended) => {}
            Ok(Execution::Returned(_)) | Err(_) => {
                self.finished = true;
                self.registers.fill(Slot::Value(Value::Null));
                self.program = None;
                if let Err(error) = result {
                    self.runner.get_mut().failure = Some(RunnerFailure::Runtime(error));
                }
            }
        }
        result
    }
    fn run(&mut self) -> Result<Execution, VmError> {
        loop {
            self.remaining = self
                .remaining
                .checked_sub(1)
                .ok_or(VmError::BudgetOverflow)?;
            if self.remaining <= 0 {
                return Ok(Execution::Suspended);
            }
            let instruction = *self
                .program()?
                .instructions
                .get(self.ip)
                .ok_or(VmError::InvalidBytecode)?;
            self.ip = self.ip.checked_add(1).ok_or(VmError::InvalidBytecode)?;
            if let Some(value) = self.step(instruction)? {
                return Ok(Execution::Returned(value));
            }
        }
    }
    fn register(&self, index: i32) -> Result<Value, VmError> {
        if index == 0 {
            return Err(VmError::InvalidBytecode);
        }
        let index = usize::try_from(index).map_err(|_| VmError::InvalidBytecode)?;
        self.registers
            .get(index)
            .ok_or(VmError::InvalidBytecode)?
            .value()
            .cloned()
    }
    fn literal(&self, index: i32) -> Result<Value, VmError> {
        let index = usize::try_from(index).map_err(|_| VmError::InvalidBytecode)?;
        self.program()?
            .literals
            .get(index)
            .cloned()
            .ok_or(VmError::InvalidBytecode)
    }
    fn write(&mut self, index: u8, value: Value) -> Result<(), VmError> {
        if index == 0 {
            return Err(VmError::InvalidBytecode);
        }
        *self
            .registers
            .get_mut(usize::from(index))
            .ok_or(VmError::InvalidBytecode)? = Slot::Value(value);
        Ok(())
    }
    fn jump(&mut self, offset: i32) -> Result<(), VmError> {
        let offset = isize::try_from(offset).map_err(|_| VmError::InvalidBytecode)?;
        let target = self
            .ip
            .checked_add_signed(offset)
            .ok_or(VmError::InvalidBytecode)?;
        if target >= self.program()?.instructions.len() {
            return Err(VmError::InvalidBytecode);
        }
        self.ip = target;
        Ok(())
    }
    fn scope_end(&mut self, instruction: Instruction) -> Result<(), VmError> {
        let from = usize::from(instruction.arg0);
        let count = instruction
            .arg1
            .checked_sub(i32::from(instruction.arg0))
            .and_then(|n| n.checked_add(2))
            .ok_or(VmError::InvalidBytecode)?;
        if from == 0 || from > self.registers.len() {
            return Err(VmError::InvalidBytecode);
        }
        // Nested-loop native last_stacksize may outlive inner local scopes.
        // A nonpositive signed cleanup count performs no stores in the native loop.
        if count <= 0 {
            return Ok(());
        }
        let count = usize::try_from(count).map_err(|_| VmError::InvalidBytecode)?;
        let end = from.checked_add(count).ok_or(VmError::InvalidBytecode)?;
        if from == 0 {
            return Err(VmError::InvalidBytecode);
        }
        if end <= self.registers.len() {
            self.registers
                .get_mut(from..end)
                .ok_or(VmError::InvalidBytecode)?
                .fill(Slot::Value(Value::Null));
        }
        Ok(())
    }
    fn return_value(&mut self, instruction: Instruction) -> Result<Value, VmError> {
        let value = if instruction.arg0 == 255 {
            Value::Null
        } else {
            self.register(instruction.arg1)?
        };
        self.store_temporary(value)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod root_sessions;

#[cfg(test)]
mod array_sessions;
