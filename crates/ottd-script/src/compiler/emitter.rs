//! Native peepholes and explicit optimization barriers.
use super::registers::Registers;
use crate::Instruction;
pub(super) struct Emitter {
    pub instructions: Vec<Instruction>,
    enabled: bool,
}
impl Emitter {
    pub(super) const fn new() -> Self {
        Self {
            instructions: Vec::new(),
            enabled: true,
        }
    }
    pub(super) const fn barrier(&mut self) {
        self.enabled = false;
    }
    pub(super) fn emit(&mut self, instruction: Instruction, registers: &Registers<'_>) {
        if self.enabled {
            if let Some(previous) = self.instructions.last_mut() {
                match instruction.opcode {
                    0x20 if previous.opcode == 0x01
                        && i32::from(previous.arg0) == instruction.arg1
                        && !registers.is_local(previous.arg0) =>
                    {
                        previous.opcode = 0x20;
                        previous.arg0 = instruction.arg0;
                        previous.arg2 = 255;
                        previous.arg3 = 255;
                        return;
                    }
                    0x0a if matches!(previous.opcode, 0x0e | 0x11 | 0x12)
                        && i32::from(previous.arg0) == instruction.arg1 =>
                    {
                        previous.arg0 = instruction.arg0;
                        self.enabled = false;
                        return;
                    }
                    0x0e if previous.opcode == 0x01
                        && previous.arg0 == instruction.arg2
                        && !registers.is_local(previous.arg0) =>
                    {
                        if let Ok(receiver) = u8::try_from(instruction.arg1) {
                            previous.opcode = 0x09;
                            previous.arg0 = instruction.arg0;
                            previous.arg2 = receiver;
                            return;
                        }
                    }
                    0x0a if previous.opcode == 0x0a => {
                        if let Ok(source) = u8::try_from(instruction.arg1) {
                            previous.opcode = 0x17;
                            previous.arg2 = instruction.arg0;
                            previous.arg3 = source;
                            return;
                        }
                    }
                    0x01 if previous.opcode == 0x01 => {
                        if let Ok(index) = u8::try_from(instruction.arg1) {
                            previous.opcode = 0x04;
                            previous.arg2 = instruction.arg0;
                            previous.arg3 = index;
                            return;
                        }
                    }
                    0x0f | 0x10
                        if previous.opcode == 0x01
                            && i32::from(previous.arg0) == instruction.arg1
                            && !registers.is_local(previous.arg0) =>
                    {
                        previous.opcode = instruction.opcode;
                        previous.arg0 = instruction.arg0;
                        previous.arg2 = instruction.arg2;
                        previous.arg3 = 255;
                        return;
                    }
                    0x14 if previous.opcode == 0x14
                        && i32::from(previous.arg0).checked_add(previous.arg1)
                            == Some(i32::from(instruction.arg0)) =>
                    {
                        if let Some(count) = previous.arg1.checked_add(1) {
                            previous.arg1 = count;
                            return;
                        }
                    }
                    _ => {}
                }
            }
        }
        self.enabled = true;
        self.instructions.push(instruction);
    }
    pub(super) fn position(&self) -> Option<i32> {
        i32::try_from(self.instructions.len()).ok()?.checked_sub(1)
    }
    pub(super) fn patch(&mut self, position: i32, offset: i32) -> Option<()> {
        self.instructions
            .get_mut(usize::try_from(position).ok()?)?
            .arg1 = offset;
        Some(())
    }
}
