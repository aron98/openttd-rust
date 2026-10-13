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
    fn bit_not(&mut self, source: i32) -> Result<Value, VmError> {
        let value = self.register(source)?;
        if let Value::Integer(integer) = value {
            return Ok(Value::Integer(!integer));
        }
        let message = format!(
            "attempt to perform a bitwise op on a {}",
            value.type_label()
        );
        Err(self.native_failure(message.as_bytes(), VmError::OperandType))
    }
    fn arithmetic_temporary(&mut self, i: Instruction) -> Result<Value, VmError> {
        let value = self.arithmetic_values(
            &self.register(i32::from(i.arg2))?,
            &self.register(i.arg1)?,
            i.arg3,
        )?;
        self.store_temporary(value)
    }
    pub(super) fn step(&mut self, i: Instruction) -> Result<Option<Value>, VmError> {
        if matches!(i.opcode, 0x38..=0x3a) {
            self.trap_instruction(i)?;
            return Ok(None);
        }
        let value = match i.opcode {
            0x05 | 0x06 => {
                self.dispatch_call(i)?;
                return Ok(None);
            }
            0x21 => self.parent_value(i)?,
            0x35 => self.attach_delegate(i)?,
            0x29 => self.membership(i)?,
            0x30 => self.create_closure(i)?,
            0x1b => self.load_outer(i.arg1)?,
            0x1c | 0x1d => self.vararg(i)?,
            0x07 | 0x08 => {
                self.prepare_call(i)?;
                return Ok(None);
            }
            0x33 => {
                self.foreach(i)?;
                return Ok(None);
            }
            0x0c => self.delete_slot(i)?,
            0x36 => self.clone_object(i)?,
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
            0x15 => Value::Table(self.runner.get().root.as_ref().table()),
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
            0x12 | 0x28 => self.binary_operation(i)?,
            0x22 => {
                self.compound_field(i)?;
                return Ok(None);
            }
            0x23 | 0x25 | 0x27 => {
                self.update(i)?;
                return Ok(None);
            }
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
            0x11 => self.arithmetic_temporary(i)?,
            0x13 => return self.frame_return(i),
            0x14 => {
                self.load_nulls(i)?;
                return Ok(None);
            }
            0x16 => Value::Bool(i.arg1 != 0),
            0x37 => self.typeof_value(i.arg1)?,
            0x2d => self.negate_value(&self.register(i.arg1)?)?,
            0x2e => Value::Bool(self.register(i.arg1)?.is_false()),
            0x2f => self.bit_not(i.arg1)?,
            opcode => return Err(VmError::UnsupportedOpcode(opcode)),
        };
        self.write(i.arg0, value)?;
        Ok(None)
    }
}
