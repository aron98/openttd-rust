//! Branch patching and loop exit ownership.
use super::super::Compiler;
use crate::{CompileError, CompileErrorKind, Instruction, lexer::TokenKind};
impl Compiler<'_> {
    pub(super) fn conditional(&mut self, depth: u8) -> Result<(), CompileError> {
        self.advance()?;
        self.expect(b'(')?;
        self.comma(0)?;
        self.expect(b')')?;
        let condition = self.pop()?;
        self.emit(Instruction {
            opcode: 0x1a,
            arg0: condition.0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        let branch = self.position()?;
        let size = self.size()?;
        self.statement(depth)?;
        if !matches!(self.token.kind, TokenKind::Symbol(b'}') | TokenKind::Else) {
            self.semicolon()?;
        }
        self.registers.truncate(size);
        let mut end_first = self.position()?;
        if self.token.kind == TokenKind::Else {
            self.emit(Instruction {
                opcode: 0x18,
                arg0: 0,
                arg1: 0,
                arg2: 0,
                arg3: 0,
            });
            let skip = self.position()?;
            self.advance()?;
            self.statement(depth)?;
            self.semicolon()?;
            self.registers.truncate(size);
            self.patch(skip, self.position()?)?;
            end_first = end_first
                .checked_add(1)
                .ok_or_else(|| self.error(CompileErrorKind::Limit))?;
        }
        self.patch(branch, end_first)?;
        Ok(())
    }
    pub(super) fn while_loop(&mut self, depth: u8) -> Result<(), CompileError> {
        let head = self.position()?;
        self.advance()?;
        self.expect(b'(')?;
        self.comma(0)?;
        self.expect(b')')?;
        let condition = self.pop()?;
        self.emit(Instruction {
            opcode: 0x1a,
            arg0: condition.0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        let branch = self.position()?;
        let size = self.size()?;
        self.begin_loop()?;
        self.statement(depth)?;
        self.registers.truncate(size);
        self.emit(Instruction {
            opcode: 0x18,
            arg0: 0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        self.patch(self.position()?, head)?;
        let end = self.position()?;
        self.patch(branch, end)?;
        self.finish_loop(head)
    }
    pub(super) fn loop_exit(&mut self, continuing: bool) -> Result<(), CompileError> {
        let missing = if continuing {
            self.continue_targets.is_empty()
        } else {
            self.break_targets.is_empty()
        };
        if missing {
            return Err(self.error(CompileErrorKind::ExpectedToken));
        }
        self.scope_end(self.last_stack_size)?;
        self.emit(Instruction {
            opcode: 0x18,
            arg0: 0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        let position = self.position()?;
        let error = self.error(CompileErrorKind::ExpectedToken);
        let targets = if continuing {
            self.continue_targets.last_mut()
        } else {
            self.break_targets.last_mut()
        };
        targets.ok_or(error)?.push(position);
        self.advance()
    }
}
