//! Native call targets preserve local aliases and snapshot member receivers.
use super::{Compiler, ExpressionState, Register};
use crate::compiler::diagnostics::CompileFailure;
use crate::{CompileErrorKind, Instruction, lexer::TokenKind};
impl Compiler<'_> {
    pub(super) fn function_call(
        &mut self,
        depth: u8,
        state: &mut ExpressionState,
    ) -> Result<(), CompileFailure> {
        if state.field && state.dereference.is_some() {
            let key = self.pop()?;
            let receiver = self.pop()?;
            let closure = self.push()?;
            let this = self.push()?;
            self.emit(Instruction {
                opcode: 0x07,
                arg0: closure.0,
                arg1: i32::from(key.0),
                arg2: receiver.0,
                arg3: this.0,
            });
        } else {
            let this = self.push()?;
            self.move_to(this, Register(0));
        }
        state.dereference = None;
        state.field = false;
        self.advance()?;
        let mut count = 1_u8;
        while self.token.kind != TokenKind::Symbol(b')') {
            let _argument = self.expression(self.depth(depth)?)?;
            let top = self
                .registers
                .top()
                .ok_or_else(|| self.error(CompileErrorKind::ExpectedToken))?;
            if self.registers.is_local(top.0) {
                let source = self.pop()?;
                let target = self.push()?;
                self.move_to(target, source);
            }
            count = count
                .checked_add(1)
                .ok_or_else(|| self.error(CompileErrorKind::Limit))?;
            if self.token.kind == TokenKind::Symbol(b',') {
                self.advance()?;
                if self.token.kind == TokenKind::Symbol(b')') {
                    return Err(self
                        .error(CompileErrorKind::ExpectedToken)
                        .native("expression expected, found ')'"));
                }
            }
        }
        self.advance()?;
        for _ in 1..count {
            let _argument = self.pop()?;
        }
        let base = self.pop()?;
        let closure = self.pop()?;
        let target = self.push()?;
        self.emit(Instruction {
            opcode: 0x06,
            arg0: target.0,
            arg1: i32::from(closure.0),
            arg2: base.0,
            arg3: count,
        });
        Ok(())
    }
}
