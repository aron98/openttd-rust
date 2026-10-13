//! Delegate operands use `CommaExpr` order; parent access is a distinct native opcode.
use super::{Compiler, ExpressionState, Register};
use crate::{
    CompileErrorKind, Instruction, compiler::diagnostics::CompileFailure, lexer::TokenKind,
};

impl Compiler<'_> {
    pub(super) fn delegate_expression(
        &mut self,
        depth: u8,
    ) -> Result<Option<Register>, CompileFailure> {
        self.advance()?;
        self.comma(depth)?;
        self.expect(b':')?;
        self.comma(depth)?;
        let table = self.pop()?;
        let delegate = self.pop()?;
        let target = self.push()?;
        self.emit(Instruction {
            opcode: 0x35,
            arg0: target.0,
            arg1: i32::from(table.0),
            arg2: delegate.0,
            arg3: 0,
        });
        Ok(None)
    }
    pub(super) fn bare_parent(&mut self) -> Result<Option<Register>, CompileFailure> {
        self.advance()?;
        self.parent_expression(Register(0))?;
        Ok(None)
    }
    pub(super) fn parent_expression(
        &mut self,
        receiver: Register,
    ) -> Result<Register, CompileFailure> {
        let target = self.push()?;
        self.emit(Instruction {
            opcode: 0x21,
            arg0: target.0,
            arg1: i32::from(receiver.0),
            arg2: 0,
            arg3: 0,
        });
        Ok(target)
    }
    pub(super) fn parent_field(
        &mut self,
        state: &mut ExpressionState,
    ) -> Result<(), CompileFailure> {
        self.advance()?;
        if matches!(
            self.token.kind,
            TokenKind::Symbol(b'=' | b'(')
                | TokenKind::NewSlot
                | TokenKind::Compound(_)
                | TokenKind::Increment(_)
        ) || (state.deleting && !matches!(self.token.kind, TokenKind::Symbol(b'.' | b'[')))
        {
            return Err(self
                .error(CompileErrorKind::UnsupportedSyntax)
                .native("parent cannot be set"));
        }
        let receiver = self.pop()?;
        let target = self.parent_expression(receiver)?;
        state.dereference = Some(target);
        state.field = true;
        state.capture = None;
        Ok(())
    }
}
