use crate::{Instruction, Program, Value, VmError};

/// Outcome of an operation-budget slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Execution {
    /// Budget exhausted before fetching the next instruction.
    Suspended,
    /// Main function returned a scalar.
    Returned(Value),
}
/// Register VM retaining instruction pointer, registers and operation debt.
#[derive(Debug)]
pub struct Vm<'a> {
    program: &'a Program,
    registers: Vec<Value>,
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
        if !(1..=255).contains(&program.stack_size) {
            return Err(VmError::InvalidBytecode);
        }
        Ok(Self {
            program,
            registers: vec![Value::Null; usize::from(program.stack_size)],
            ip: 0,
            remaining: 0,
            finished: false,
        })
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
        let result = self.run();
        match result {
            Ok(Execution::Suspended) => {}
            Ok(Execution::Returned(_)) | Err(_) => self.finished = true,
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
                .program
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
            .copied()
            .ok_or(VmError::InvalidBytecode)
    }
    fn literal(&self, index: i32) -> Result<Value, VmError> {
        let index = usize::try_from(index).map_err(|_| VmError::InvalidBytecode)?;
        self.program
            .literals
            .get(index)
            .copied()
            .ok_or(VmError::InvalidBytecode)
    }
    fn write(&mut self, index: u8, value: Value) -> Result<(), VmError> {
        if index == 0 {
            return Err(VmError::InvalidBytecode);
        }
        *self
            .registers
            .get_mut(usize::from(index))
            .ok_or(VmError::InvalidBytecode)? = value;
        Ok(())
    }
    fn step(&mut self, i: Instruction) -> Result<Option<Value>, VmError> {
        let value = match i.opcode {
            0x01 => self.literal(i.arg1)?,
            0x02 => Value::Integer(i64::from(i.arg1)),
            0x03 => Value::Float(u32::from_ne_bytes(i.arg1.to_ne_bytes())),
            0x04 => {
                let first = self.literal(i.arg1)?;
                let second = self.literal(i32::from(i.arg3))?;
                self.write(i.arg0, first)?;
                self.write(i.arg2, second)?;
                return Ok(None);
            }
            0x11 => self
                .register(i32::from(i.arg2))?
                .arithmetic(self.register(i.arg1)?, i.arg3)?,
            0x13 => {
                return Ok(Some(if i.arg0 == 255 {
                    Value::Null
                } else {
                    self.register(i.arg1)?
                }));
            }
            0x14 => {
                let length = usize::try_from(i.arg1).map_err(|_| VmError::InvalidBytecode)?;
                let start = usize::from(i.arg0);
                if start == 0 {
                    return Err(VmError::InvalidBytecode);
                }
                let end = start.checked_add(length).ok_or(VmError::InvalidBytecode)?;
                self.registers
                    .get_mut(start..end)
                    .ok_or(VmError::InvalidBytecode)?
                    .fill(Value::Null);
                return Ok(None);
            }
            0x16 => Value::Bool(i.arg1 != 0),
            0x2d => self.register(i.arg1)?.negate()?,
            0x2e => Value::Bool(self.register(i.arg1)?.is_false()),
            0x2f => match self.register(i.arg1)? {
                Value::Integer(n) => Value::Integer(!n),
                Value::Null | Value::Float(_) | Value::Bool(_) => return Err(VmError::OperandType),
            },
            opcode => return Err(VmError::UnsupportedOpcode(opcode)),
        };
        self.write(i.arg0, value)?;
        Ok(None)
    }
}
