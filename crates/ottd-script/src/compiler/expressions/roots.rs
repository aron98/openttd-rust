//! Main-frame root fields retain native receiver/key targets until a store.
use super::{Compiler, ExpressionState, Register};
use crate::{CompileError, CompileErrorKind, Instruction, Value, lexer::TokenKind};
impl Compiler<'_> {
    pub(super) fn explicit_root(
        &mut self,
        state: &mut ExpressionState,
    ) -> Result<Option<Register>, CompileError> {
        self.advance()?;
        let TokenKind::Identifier(name) = self.token.kind else {
            return Err(self.error(CompileErrorKind::UnsupportedSyntax));
        };
        self.advance()?;
        let root = self.push()?;
        self.emit(Instruction {
            opcode: 0x15,
            arg0: root.0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        self.root_field(name, state)
    }
    pub(super) fn root_field(
        &mut self,
        name: &str,
        state: &mut ExpressionState,
    ) -> Result<Option<Register>, CompileError> {
        let key = self.push()?;
        self.load(key, Value::String(self.realm.string(name.as_bytes())))?;
        if matches!(
            self.token.kind,
            TokenKind::Compound(_) | TokenKind::Increment(_) | TokenKind::Symbol(b'(' | b'.')
        ) {
            return Err(self.error(CompileErrorKind::UnsupportedSyntax));
        }
        if !matches!(
            self.token.kind,
            TokenKind::Symbol(b'=') | TokenKind::NewSlot
        ) {
            let key = self.pop()?;
            let root = self.pop()?;
            let target = self.push()?;
            self.emit(Instruction {
                opcode: 0x0e,
                arg0: target.0,
                arg1: i32::from(root.0),
                arg2: key.0,
                arg3: 0,
            });
        }
        state.dereference = Some(key);
        state.field = true;
        Ok(None)
    }
}
