//! Single native register opcode dispatch.
use super::{Instruction, Value, Vm, VmError};
impl Vm<'_> {
    fn equality(&self, i: Instruction) -> Result<Value, VmError> {
        let right = if i.arg3 == 0 {
            self.register(i.arg1)?
        } else {
            self.literal(i.arg1)?
        };
        let equal = self.register(i32::from(i.arg2))?.equal(&right)?;
        Ok(Value::Bool(if i.opcode == 0x0f { equal } else { !equal }))
    }
    fn bit_not(&self, source: i32) -> Result<Value, VmError> {
        match self.register(source)? {
            Value::Integer(value) => Ok(Value::Integer(!value)),
            Value::Null
            | Value::Float(_)
            | Value::Bool(_)
            | Value::String(_)
            | Value::Array(_)
            | Value::Table(_) => Err(VmError::OperandType),
        }
    }
    pub(super) fn step(&mut self, i: Instruction) -> Result<Option<Value>, VmError> {
        let value = match i.opcode {
            0x33 => {
                self.foreach(i)?;
                return Ok(None);
            }
            0x1e => self.new_table(i)?,
            0x1f => self.new_array(i)?,
            0x20 => {
                self.append_array(i)?;
                return Ok(None);
            }
            0x09 | 0x0e => self.root_get(i)?,
            0x0b | 0x0d | 0x3c => {
                self.root_store(i)?;
                return Ok(None);
            }
            0x15 => {
                self.write_root(i.arg0, self.runner.get().root.clone())?;
                return Ok(None);
            }
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
            0x0a => self.register(i.arg1)?,
            0x0f | 0x10 => self.equality(i)?,
            0x17 => {
                self.write(i.arg0, self.register(i.arg1)?)?;
                self.write(i.arg2, self.register(i32::from(i.arg3))?)?;
                return Ok(None);
            }
            0x18 => {
                self.jump(i.arg1)?;
                return Ok(None);
            }
            0x19 | 0x1a => {
                if self.register(i32::from(i.arg0))?.is_false() == (i.opcode == 0x1a) {
                    self.jump(i.arg1)?;
                }
                return Ok(None);
            }
            0x12 => self
                .register(i32::from(i.arg2))?
                .bitwise(self.register(i.arg1)?, i.arg3)?,
            0x23 | 0x25 | 0x27 => {
                self.update(i)?;
                return Ok(None);
            }
            0x28 => self
                .register(i32::from(i.arg2))?
                .compare(&self.register(i.arg1)?, i.arg3)?,
            0x2b | 0x2c => {
                let source = self.register(i32::from(i.arg2))?;
                if source.is_false() == (i.opcode == 0x2b) {
                    self.write(i.arg0, source)?;
                    self.jump(i.arg1)?;
                }
                return Ok(None);
            }
            0x3d => {
                self.scope_end(i)?;
                return Ok(None);
            }
            0x11 => {
                let value = self.register(i32::from(i.arg2))?.arithmetic(
                    &self.register(i.arg1)?,
                    i.arg3,
                    &self.runner.get().root.realm,
                )?;
                self.store_temporary(value)?
            }
            0x13 => return self.return_value(i).map(Some),
            0x14 => {
                self.load_nulls(i)?;
                return Ok(None);
            }
            0x16 => Value::Bool(i.arg1 != 0),
            0x37 => self
                .register(i.arg1)?
                .type_name(&self.runner.get().root.realm),
            0x2d => self.register(i.arg1)?.negate()?,
            0x2e => Value::Bool(self.register(i.arg1)?.is_false()),
            0x2f => self.bit_not(i.arg1)?,
            opcode => return Err(VmError::UnsupportedOpcode(opcode)),
        };
        self.write(i.arg0, value)?;
        Ok(None)
    }
}
