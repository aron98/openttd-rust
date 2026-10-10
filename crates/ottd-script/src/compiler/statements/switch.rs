//! Switch retains selector/case targets and owns breaks but not continues.
use super::super::Compiler;
use crate::{CompileError, Instruction, lexer::TokenKind};
impl Compiler<'_> {
    pub(super) fn switch_statement(&mut self, depth: u8) -> Result<(), CompileError> {
        self.advance()?;
        self.expect(b'(')?;
        self.comma(0)?;
        self.expect(b')')?;
        self.expect(b'{')?;
        let selector = self
            .registers
            .top()
            .ok_or_else(|| self.error(crate::CompileErrorKind::ExpectedToken))?;
        self.break_targets.push(Vec::new());
        let mut next_condition = None;
        while self.token.kind == TokenKind::Case {
            let skip_condition = if let Some(previous) = next_condition {
                self.emit(Instruction {
                    opcode: 0x18,
                    arg0: 0,
                    arg1: 0,
                    arg2: 0,
                    arg3: 0,
                });
                let skip = self.position()?;
                self.patch(previous, skip)?;
                Some(skip)
            } else {
                None
            };
            self.advance()?;
            let _state = self.expression(0)?;
            self.expect(b':')?;
            let target = self.pop()?;
            self.emit(Instruction {
                opcode: 0x0f,
                arg0: target.0,
                arg1: i32::from(target.0),
                arg2: selector.0,
                arg3: 0,
            });
            self.emit(Instruction {
                opcode: 0x1a,
                arg0: target.0,
                arg1: 0,
                arg2: 0,
                arg3: 0,
            });
            let branch = self.position()?;
            if let Some(skip) = skip_condition {
                self.patch(skip, branch)?;
            }
            next_condition = Some(branch);
            self.switch_body(depth)?;
        }
        if let Some(branch) = next_condition {
            self.patch(branch, self.position()?)?;
        }
        if self.token.kind == TokenKind::Default {
            self.advance()?;
            self.expect(b':')?;
            self.switch_body(depth)?;
        }
        self.expect(b'}')?;
        let _selector = self.pop()?;
        self.finish_breaks()
    }
    fn switch_body(&mut self, depth: u8) -> Result<(), CompileError> {
        let size = self.size()?;
        self.last_stack_size = size;
        self.statements(depth)?;
        self.registers.truncate(size);
        Ok(())
    }
}
