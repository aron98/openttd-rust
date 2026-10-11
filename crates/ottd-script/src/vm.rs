use crate::{Instruction, Program, Runner, RunnerFailure, Storage, Temporary, Value, VmError};
#[expect(
    clippy::redundant_pub_crate,
    reason = "Native numeric conversion is shared with table hashing"
)]
pub(crate) mod arrays;
mod calls;
mod closures;
mod diagnostics;
mod dispatch;
mod foreach;
mod frames;
mod objects;
mod roots;
mod tables;
mod updates;
mod varargs;
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
    program: Option<crate::ScriptClosure>,
    _host: crate::HostStack,
    origin: usize,
    base: usize,
    top: usize,
    frames: Vec<frames::ParentFrame>,
    varargs: varargs::VarArgs,
    varargs_stack: Vec<Value>,
    native_scratch: Value,
    registers: Vec<Value>,
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
            Storage::Owned(Runner::new(program.data.0.realm.default_root()?)),
        )
    }
    pub(crate) fn with_runner(program: &Program, runner: Storage<'a>) -> Result<Self, VmError> {
        Self::with_host_runner(program, runner, crate::HostStack::new())
    }
    pub(crate) fn with_host_runner(
        program: &Program,
        runner: Storage<'a>,
        host: crate::HostStack,
    ) -> Result<Self, VmError> {
        if !host.same_realm(&program.data.0.realm) {
            return Err(VmError::RealmMismatch);
        }
        if !(1..=255).contains(&program.stack_size()) {
            return Err(VmError::InvalidBytecode);
        }
        let mut registers = vec![Value::Null; usize::from(program.stack_size())];
        *registers.first_mut().ok_or(VmError::InvalidBytecode)? =
            Value::Table(runner.get().root.table());
        Ok(Self {
            program: Some(program.data.clone()),
            origin: host.len(),
            base: 0,
            top: registers.len(),
            frames: Vec::new(),
            varargs: varargs::VarArgs::default(),
            varargs_stack: Vec::new(),
            _host: host,
            native_scratch: Value::Null,
            registers,
            runner,
            started: false,
            ip: 0,
            remaining: 0,
            finished: false,
        })
    }
    /// Start a fresh bare-root frame above real retained host entries.
    /// # Errors
    /// Rejects foreign entries, invalid stack shape, or root allocation failure.
    pub fn with_host(program: &'a Program, host: crate::HostStack) -> Result<Self, VmError> {
        Self::with_host_runner(
            program,
            Storage::Owned(Runner::new(program.data.0.realm.default_root()?)),
            host,
        )
    }
    /// Last native-call error bytes, including a failure in this active frame.
    pub const fn native_error(&self) -> Option<&crate::ByteString> {
        self.runner.get().native_error()
    }
    fn program(&self) -> Result<crate::Prototype, VmError> {
        self.program
            .as_ref()
            .map(crate::ScriptClosure::prototype)
            .ok_or(VmError::Finished)
    }
    /// Index of the next instruction; meaningful at suspension boundaries.
    pub const fn instruction_pointer(&self) -> usize {
        self.ip
    }
    /// Independently retained metadata of the current script frame.
    pub fn active_prototype(&self) -> Option<crate::Prototype> {
        self.program.as_ref().map(crate::ScriptClosure::prototype)
    }
    /// Current absolute stack base, top, and active script frame count.
    pub fn frame_position(&self) -> Option<(usize, usize, usize)> {
        Some((
            self.origin.checked_add(self.base)?,
            self.origin.checked_add(self.top)?,
            self.frames.len().checked_add(1)?,
        ))
    }
    /// Remaining operations, including pre-dispatch suspension debt.
    pub const fn remaining_ops(&self) -> i64 {
        self.remaining
    }
    /// Retained unnamed arguments and the current frame's base and length.
    /// Borrowing this view does not add ownership references to the values.
    pub fn vararg_state(&self) -> (&[Value], usize, usize) {
        (&self.varargs_stack, self.varargs.base, self.varargs.len)
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
            let program = self.program.as_ref().ok_or(VmError::Finished)?.clone();
            self.runner.get_mut().temporary = Temporary::MainProgram { _program: program };
            self.started = true;
        }
        let result = self.run();
        match result {
            Ok(Execution::Suspended) => {}
            Ok(Execution::Returned(_)) | Err(_) => {
                self.finished = true;
                self.registers.fill(Value::Null);
                self.native_scratch = Value::Null;
                self.program = None;
                self.frames.clear();
                self.varargs_stack.clear();
                self.varargs = varargs::VarArgs::default();
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
                .0
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
        let index = usize::try_from(index).map_err(|_| VmError::InvalidBytecode)?;
        let index = self
            .base
            .checked_add(index)
            .ok_or(VmError::InvalidBytecode)?;
        if index >= self.top {
            return Err(VmError::InvalidBytecode);
        }
        self.registers
            .get(index)
            .cloned()
            .ok_or(VmError::InvalidBytecode)
    }
    fn literal(&self, index: i32) -> Result<Value, VmError> {
        let index = usize::try_from(index).map_err(|_| VmError::InvalidBytecode)?;
        self.program()?
            .0
            .literals
            .get(index)
            .map(crate::table::scalar::Scalar::value)
            .ok_or(VmError::InvalidBytecode)
    }
    fn write(&mut self, index: u8, value: Value) -> Result<(), VmError> {
        let index = self
            .base
            .checked_add(usize::from(index))
            .ok_or(VmError::InvalidBytecode)?;
        if index >= self.top {
            return Err(VmError::InvalidBytecode);
        }
        *self
            .registers
            .get_mut(index)
            .ok_or(VmError::InvalidBytecode)? = value;
        Ok(())
    }
    fn jump(&mut self, offset: i32) -> Result<(), VmError> {
        let offset = isize::try_from(offset).map_err(|_| VmError::InvalidBytecode)?;
        let target = self
            .ip
            .checked_add_signed(offset)
            .ok_or(VmError::InvalidBytecode)?;
        if target >= self.program()?.0.instructions.len() {
            return Err(VmError::InvalidBytecode);
        }
        self.ip = target;
        Ok(())
    }
    fn scope_end(&mut self, instruction: Instruction) -> Result<(), VmError> {
        let from = self
            .base
            .checked_add(usize::from(instruction.arg0))
            .ok_or(VmError::InvalidBytecode)?;
        let count = instruction
            .arg1
            .checked_sub(i32::from(instruction.arg0))
            .and_then(|n| n.checked_add(2))
            .ok_or(VmError::InvalidBytecode)?;
        if from == 0 || from > self.top {
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
        if end <= self.top {
            self.registers
                .get_mut(from..end)
                .ok_or(VmError::InvalidBytecode)?
                .fill(Value::Null);
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

#[cfg(test)]
mod native_roots;

#[cfg(test)]
mod native_calls;
