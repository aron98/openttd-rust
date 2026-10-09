//! Scalar expression lowering with native local aliasing and precedence.
use super::{Compiler, Register};
use crate::{CompileError, CompileErrorKind, Instruction, lexer::TokenKind};
/// One native PushExpState/PopExpState lifetime, owned by a recursive expression.
/// Operators share this state; parenthesized expressions create a separate owner.
#[derive(Default)]
pub(super) struct ExpressionState {
    dereference: Option<Register>,
}
impl Compiler<'_> {
    pub(super) fn expression(&mut self, depth: u8) -> Result<ExpressionState, CompileError> {
        self.depth(depth)?;
        let mut state = ExpressionState::default();
        self.logical(0, depth, &mut state)?;
        if self.token.kind == TokenKind::Symbol(b'=') {
            if state.dereference.is_none() {
                return Err(self.error(CompileErrorKind::UnsupportedSyntax));
            }
            self.advance()?;
            let _rhs = self.expression(self.depth(depth)?)?;
            let source = self.pop()?;
            let destination = self
                .registers
                .top()
                .ok_or_else(|| self.error(CompileErrorKind::ExpectedToken))?;
            self.emit(Instruction {
                opcode: 0x0a,
                arg0: destination.0,
                arg1: i32::from(source.0),
                arg2: 0,
                arg3: 0,
            });
        }
        Ok(state)
    }
    fn logical(
        &mut self,
        level: u8,
        depth: u8,
        state: &mut ExpressionState,
    ) -> Result<(), CompileError> {
        self.depth(depth)?;
        if level == 0 {
            self.logical(1, depth, state)?;
        } else {
            self.binary(0, depth, state)?;
        }
        let token = if level == 0 {
            TokenKind::Or
        } else {
            TokenKind::And
        };
        if self.token.kind != token {
            return Ok(());
        }
        let first = self.pop()?;
        let target = self.push()?;
        self.emit(Instruction {
            opcode: if level == 0 { 0x2c } else { 0x2b },
            arg0: target.0,
            arg1: 0,
            arg2: first.0,
            arg3: 0,
        });
        let branch = self.position()?;
        if target != first {
            self.move_to(target, first);
        }
        self.advance()?;
        self.logical(level, self.depth(depth)?, state)?;
        self.emitter.barrier();
        let second = self.pop()?;
        if target != second {
            self.move_to(target, second);
        }
        self.emitter.barrier();
        self.patch(branch, self.position()?)?;
        Ok(())
    }
    fn binary(
        &mut self,
        precedence: u8,
        depth: u8,
        state: &mut ExpressionState,
    ) -> Result<(), CompileError> {
        let depth = self.depth(depth)?;
        self.factor(depth, state)?;
        while let Some((rank, opcode, operation)) = self.token.kind.binary() {
            if rank <= precedence {
                break;
            }
            self.advance()?;
            self.binary(rank, depth, state)?;
            let right = self.pop()?;
            let left = self.pop()?;
            let target = self.push()?;
            self.emit(Instruction {
                opcode,
                arg0: target.0,
                arg1: i32::from(right.0),
                arg2: left.0,
                arg3: operation,
            });
        }
        Ok(())
    }
    fn factor(&mut self, depth: u8, state: &mut ExpressionState) -> Result<(), CompileError> {
        let depth = self.depth(depth)?;
        state.dereference = None;
        match self.token.kind {
            TokenKind::Scalar(value) => {
                let target = self.push()?;
                self.load(target, value)?;
                self.advance()?;
                Ok(())
            }
            TokenKind::Identifier(name) => {
                let register = self
                    .registers
                    .local(name)
                    .ok_or_else(|| self.error(CompileErrorKind::UnsupportedSyntax))?;
                self.advance()?;
                self.registers.reference(register);
                state.dereference = Some(register);
                Ok(())
            }
            TokenKind::Symbol(b'(') => {
                self.advance()?;
                let _nested = self.expression(depth)?;
                self.expect(b')')?;
                Ok(())
            }
            TokenKind::Symbol(operator @ (b'-' | b'!' | b'~')) => {
                self.advance()?;
                self.factor(depth, state)?;
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
                Ok(())
            }
            TokenKind::Return
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
    pub(super) fn move_to(&mut self, destination: Register, source: Register) {
        self.emit(Instruction {
            opcode: 0x0a,
            arg0: destination.0,
            arg1: i32::from(source.0),
            arg2: 0,
            arg3: 0,
        });
    }
}
