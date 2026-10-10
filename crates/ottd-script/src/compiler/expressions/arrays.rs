//! Native allocation and indexed expression-target lowering.
use super::{Compiler, ExpressionState, Register};
use crate::{CompileError, CompileErrorKind, Instruction, lexer::TokenKind};
impl Compiler<'_> {
    pub(super) fn array_literal(&mut self, depth: u8) -> Result<Option<Register>, CompileError> {
        let array = self.push()?;
        self.emit(Instruction {
            opcode: 0x1f,
            arg0: array.0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        let position = self.position()?;
        self.advance()?;
        let mut count = 0_i32;
        while self.token.kind != TokenKind::Symbol(b']') {
            let _element = self.expression(self.depth(depth)?)?;
            if self.token.kind == TokenKind::Symbol(b',') {
                self.advance()?;
            }
            let value = self.pop()?;
            self.emit(Instruction {
                opcode: 0x20,
                arg0: array.0,
                arg1: i32::from(value.0),
                arg2: 0,
                arg3: 0,
            });
            count = count
                .checked_add(1)
                .ok_or_else(|| self.error(CompileErrorKind::Limit))?;
        }
        self.emitter
            .patch(position, count)
            .ok_or_else(|| self.error(CompileErrorKind::Limit))?;
        self.advance()?;
        Ok(None)
    }
    pub(super) fn indexed(
        &mut self,
        depth: u8,
        state: &mut ExpressionState,
    ) -> Result<(), CompileError> {
        if self.token.newline {
            return Err(self.error(CompileErrorKind::ExpectedToken));
        }
        self.advance()?;
        let _index = self.expression(self.depth(depth)?)?;
        self.expect(b']')?;
        let key = self
            .registers
            .top()
            .ok_or_else(|| self.error(CompileErrorKind::ExpectedToken))?;
        if !matches!(
            self.token.kind,
            TokenKind::Symbol(b'=')
                | TokenKind::NewSlot
                | TokenKind::Compound(_)
                | TokenKind::Increment(_)
        ) {
            let key = self.pop()?;
            let receiver = self.pop()?;
            let target = self.push()?;
            self.emit(Instruction {
                opcode: 0x0e,
                arg0: target.0,
                arg1: i32::from(receiver.0),
                arg2: key.0,
                arg3: 0,
            });
        }
        state.dereference = Some(key);
        state.field = true;
        state.constant = false;
        Ok(())
    }
}
