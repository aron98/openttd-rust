//! Factor position and postfix targeting are independent of expression dereference.
use super::{Compiler, ExpressionState, Register};
use crate::compiler::diagnostics::CompileFailure;
use crate::{CompileErrorKind, Instruction, lexer::TokenKind};
impl Compiler<'_> {
    pub(super) fn prefixed(
        &mut self,
        depth: u8,
        state: &mut ExpressionState,
    ) -> Result<(), CompileFailure> {
        let mut position = self.factor(depth, state)?;
        while matches!(self.token.kind, TokenKind::Symbol(b'[' | b'.' | b'(')) {
            if self.token.kind == TokenKind::Symbol(b'(') {
                self.function_call(depth, state)?;
            } else if self.token.kind == TokenKind::Symbol(b'.') {
                self.advance()?;
                let TokenKind::Identifier(name) = self.token.kind else {
                    return Err(self.expected_identifier());
                };
                self.advance()?;
                self.root_field(name, state)?;
            } else {
                self.indexed(depth, state)?;
            }
            position = None;
        }
        if state.field && matches!(self.token.kind, TokenKind::Increment(_)) {
            return Err(self.error(CompileErrorKind::UnsupportedSyntax));
        }
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
    ) -> Result<Option<Register>, CompileFailure> {
        let depth = self.depth(depth)?;
        state.dereference = None;
        state.field = false;
        match self.token.kind.clone() {
            TokenKind::Vargc | TokenKind::Vargv => self.vararg_expression(depth),
            TokenKind::Function => self.function_expression(depth),
            TokenKind::Scalar(value) => {
                let target = self.push()?;
                self.load(target, value)?;
                self.advance()?;
                Ok(None)
            }
            TokenKind::Identifier(name) => self.identifier(name, state),
            TokenKind::This => self.identifier("this", state),
            TokenKind::Root => self.explicit_root(state),
            TokenKind::Symbol(b'[') => self.array_literal(depth),
            TokenKind::Symbol(b'{') => self.table_literal(depth),
            TokenKind::Symbol(b'(') => {
                self.advance()?;
                self.comma(depth)?;
                self.expect(b')')?;
                Ok(None)
            }
            TokenKind::Symbol(operator @ (b'-' | b'!' | b'~')) => {
                let opcode = match operator {
                    b'-' => 0x2d,
                    b'!' => 0x2e,
                    _ => 0x2f,
                };
                self.unary_expression(depth, state, opcode)?;
                Ok(None)
            }
            TokenKind::Delete => {
                self.delete_expression(depth)?;
                Ok(None)
            }
            TokenKind::TypeOf | TokenKind::Clone => {
                let opcode = if self.token.kind == TokenKind::Clone {
                    0x36
                } else {
                    0x37
                };
                self.unary_expression(depth, state, opcode)?;
                Ok(None)
            }
            TokenKind::Increment(amount) => {
                self.advance()?;
                let mut nested = ExpressionState::default();
                self.prefixed(depth, &mut nested)?;
                if nested.field {
                    return Err(self.error(CompileErrorKind::UnsupportedSyntax));
                }
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
            TokenKind::NewSlot
            | TokenKind::Ellipsis
            | TokenKind::Const
            | TokenKind::Enum
            | TokenKind::Switch
            | TokenKind::Case
            | TokenKind::Default
            | TokenKind::For
            | TokenKind::Foreach
            | TokenKind::In
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
            | TokenKind::Try
            | TokenKind::Catch
            | TokenKind::Throw
            | TokenKind::End => Err(self
                .error(CompileErrorKind::ExpectedToken)
                .native("expression expected")),
        }
    }
    fn vararg_expression(&mut self, depth: u8) -> Result<Option<Register>, CompileFailure> {
        let indexed = self.token.kind == TokenKind::Vargv;
        self.advance()?;
        let source = if indexed {
            self.expect(b'[')?;
            self.expression(depth)?;
            self.expect(b']')?;
            i32::from(self.pop()?.0)
        } else {
            0
        };
        let target = self.push()?;
        self.emit(Instruction {
            opcode: if indexed { 0x1d } else { 0x1c },
            arg0: target.0,
            arg1: source,
            arg2: 0,
            arg3: 0,
        });
        Ok(None)
    }
    fn identifier(
        &mut self,
        name: &str,
        state: &mut ExpressionState,
    ) -> Result<Option<Register>, CompileFailure> {
        state.constant = false;
        state.capture = None;
        self.advance()?;
        if let Some(register) = self.registers.local(name) {
            self.registers.reference(register);
            state.dereference = Some(register);
            return Ok(Some(register));
        }
        if let Some(index) = self.captures.iter().position(|(found, _)| *found == name) {
            let target = self.push()?;
            self.emit(Instruction {
                opcode: 0x1b,
                arg0: target.0,
                arg1: i32::try_from(index).map_err(|_| self.error(CompileErrorKind::Limit))?,
                arg2: 0,
                arg3: 0,
            });
            state.dereference = Some(target);
            state.capture = Some(target);
            return Ok(Some(target));
        }
        let Some(binding) = self.realm.constant(name) else {
            self.registers.reference(Register(0));
            return self.root_field(name, state);
        };
        let scalar = match binding {
            crate::realm::constants::Binding::Scalar(value) => value,
            crate::realm::constants::Binding::Enum(members) => {
                self.expect(b'.')?;
                let member = self.constant_name()?;
                members
                    .get(member.as_bytes())
                    .map(|(_, value)| value.clone())
                    .ok_or_else(|| {
                        self.error(CompileErrorKind::ExpectedToken)
                            .invalid_constant(name.as_bytes(), member.as_bytes())
                    })?
            }
        };
        let register = self.push()?;
        self.load(register, scalar.value())?;
        state.dereference = Some(register);
        state.constant = true;
        Ok(None)
    }
}
