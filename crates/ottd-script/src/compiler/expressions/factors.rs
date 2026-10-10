//! Factor position and postfix targeting are independent of expression dereference.
use super::{Compiler, ExpressionState, Register};
use crate::{CompileError, CompileErrorKind, Instruction, lexer::TokenKind};
impl Compiler<'_> {
    pub(super) fn prefixed(
        &mut self,
        depth: u8,
        state: &mut ExpressionState,
    ) -> Result<(), CompileError> {
        let position = self.factor(depth, state)?;
        if let TokenKind::Increment(amount) = self.token.kind {
            if state.dereference.is_some() && !self.token.newline {
                if position.is_none() {
                    return Err(self.error(CompileErrorKind::UnsupportedSyntax));
                }
                self.advance()?;
                let source = self.pop()?;
                let target = self.push()?;
                self.emit(Instruction {
                    opcode: 0x27,
                    arg0: target.0,
                    arg1: i32::from(source.0),
                    arg2: 0,
                    arg3: amount.to_ne_bytes()[0],
                });
            }
        }
        Ok(())
    }
    fn factor(
        &mut self,
        depth: u8,
        state: &mut ExpressionState,
    ) -> Result<Option<Register>, CompileError> {
        let depth = self.depth(depth)?;
        state.dereference = None;
        match self.token.kind {
            TokenKind::Scalar(value) => {
                let target = self.push()?;
                self.load(target, value)?;
                self.advance()?;
                Ok(None)
            }
            TokenKind::Identifier(name) => {
                let register = self
                    .registers
                    .local(name)
                    .ok_or_else(|| self.error(CompileErrorKind::UnsupportedSyntax))?;
                self.advance()?;
                self.registers.reference(register);
                state.dereference = Some(register);
                Ok(Some(register))
            }
            TokenKind::Symbol(b'(') => {
                self.advance()?;
                self.comma(depth)?;
                self.expect(b')')?;
                Ok(None)
            }
            TokenKind::Symbol(operator @ (b'-' | b'!' | b'~')) => {
                self.advance()?;
                self.prefixed(depth, state)?;
                let source = self.pop()?;
                let target = self.push()?;
                let opcode = match operator {
                    b'-' => 0x2d,
                    b'!' => 0x2e,
                    _ => 0x2f,
                };
                self.emit(Instruction {
                    opcode,
                    arg0: target.0,
                    arg1: i32::from(source.0),
                    arg2: 0,
                    arg3: 0,
                });
                Ok(None)
            }
            TokenKind::Increment(amount) => {
                self.advance()?;
                let mut nested = ExpressionState::default();
                self.prefixed(depth, &mut nested)?;
                let source = self.pop()?;
                let target = self.push()?;
                self.emit(Instruction {
                    opcode: 0x25,
                    arg0: target.0,
                    arg1: i32::from(source.0),
                    arg2: 0,
                    arg3: amount.to_ne_bytes()[0],
                });
                Ok(None)
            }
            TokenKind::For
            | TokenKind::Do
            | TokenKind::Compound(_)
            | TokenKind::Shift(_)
            | TokenKind::Return
            | TokenKind::Local
            | TokenKind::If
            | TokenKind::Else
            | TokenKind::While
            | TokenKind::Break
            | TokenKind::Continue
            | TokenKind::Symbol(_)
            | TokenKind::Equal
            | TokenKind::NotEqual
            | TokenKind::LessEqual
            | TokenKind::GreaterEqual
            | TokenKind::And
            | TokenKind::Or
            | TokenKind::End => Err(self.error(CompileErrorKind::ExpectedToken)),
        }
    }
}
