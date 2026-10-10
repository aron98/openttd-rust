//! Declaration publication follows parser traversal, including failed compiles.
use super::super::Compiler;
use crate::{
    ByteString, CompileError, CompileErrorKind, Value,
    lexer::TokenKind,
    realm::constants::{Binding, Members, Scalar},
};
use std::rc::Rc;
impl Compiler<'_> {
    pub(in crate::compiler) fn constant_name(&mut self) -> Result<ByteString, CompileError> {
        let TokenKind::Identifier(name) = self.token.kind else {
            return Err(self.error(CompileErrorKind::ExpectedToken));
        };
        let name = self.realm.string(name.as_bytes());
        self.advance()?;
        Ok(name)
    }
    fn scalar(&mut self) -> Result<Scalar, CompileError> {
        let negative = self.token.kind == TokenKind::Symbol(b'-');
        if negative {
            self.advance()?;
        }
        let scalar = match self.token.kind.clone() {
            TokenKind::Scalar(Value::Integer(value)) => Scalar::Integer(if negative {
                value
                    .checked_neg()
                    .ok_or_else(|| self.error(CompileErrorKind::UnsupportedSyntax))?
            } else {
                value
            }),
            TokenKind::Scalar(Value::Float(bits)) => {
                Scalar::Float(if negative { bits ^ 0x8000_0000 } else { bits })
            }
            TokenKind::Scalar(Value::String(value)) if !negative => Scalar::String(value),
            _ => return Err(self.error(CompileErrorKind::ExpectedToken)),
        };
        self.advance()?;
        Ok(scalar)
    }
    pub(super) fn constant_declaration(&mut self) -> Result<(), CompileError> {
        self.advance()?;
        let name = self.constant_name()?;
        self.expect(b'=')?;
        let value = self.scalar()?;
        self.semicolon()?;
        self.realm.publish(name, Binding::Scalar(value));
        Ok(())
    }
    pub(super) fn enum_declaration(&mut self) -> Result<(), CompileError> {
        self.advance()?;
        let name = self.constant_name()?;
        self.expect(b'{')?;
        let mut members = Members::new();
        let mut next = 0_i64;
        while self.token.kind != TokenKind::Symbol(b'}') {
            let key = self.constant_name()?;
            let value = if self.token.kind == TokenKind::Symbol(b'=') {
                self.advance()?;
                self.scalar()?
            } else {
                let value = Scalar::Integer(next);
                next = next
                    .checked_add(1)
                    .ok_or_else(|| self.error(CompileErrorKind::Limit))?;
                value
            };
            members.insert(key.as_bytes().to_vec(), (key, value));
            if self.token.kind == TokenKind::Symbol(b',') {
                self.advance()?;
            }
        }
        self.realm.publish(name, Binding::Enum(Rc::new(members)));
        self.advance()
    }
}
