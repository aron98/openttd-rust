//! Branch patching and loop exit ownership.
use super::super::{Compiler, LoopLabels};
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
        self.loops.push(LoopLabels {
            breaks: Vec::new(),
            continues: Vec::new(),
        });
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
        self.last_stack_size = size;
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
        let labels = self
            .loops
            .pop()
            .ok_or_else(|| self.error(CompileErrorKind::ExpectedToken))?;
        for position in labels.breaks {
            self.patch(position, end)?;
        }
        for position in labels.continues {
            self.patch(position, head)?;
        }
        Ok(())
    }
    pub(super) fn loop_exit(&mut self, continuing: bool) -> Result<(), CompileError> {
        if self.loops.is_empty() {
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
        let labels = self.loops.last_mut().ok_or(error)?;
        if continuing {
            labels.continues.push(position);
        } else {
            labels.breaks.push(position);
        }
        self.advance()
    }
}
