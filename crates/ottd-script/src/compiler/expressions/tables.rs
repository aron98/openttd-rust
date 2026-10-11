//! Native table literals preserve source member capacity and NEWSLOTA lowering.
use super::{Compiler, Register};
use crate::{CompileError, CompileErrorKind, Instruction, Value, lexer::TokenKind};
impl Compiler<'_> {
    pub(super) fn table_literal(&mut self, depth: u8) -> Result<Option<Register>, CompileError> {
        let table = self.push()?;
        self.emit(Instruction {
            opcode: 0x1e,
            arg0: table.0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        let position = self.position()?;
        self.advance()?;
        let mut count = 0_i32;
        while self.token.kind != TokenKind::Symbol(b'}') {
            if self.token.kind == TokenKind::Symbol(b'[') {
                self.advance()?;
                self.comma(self.depth(depth)?)?;
                self.expect(b']')?;
            } else {
                let name = self.constant_name()?;
                let key = self.push()?;
                self.load(key, Value::String(self.realm.string(name.as_bytes())))?;
            }
            self.expect(b'=')?;
            let _value = self.expression(self.depth(depth)?)?;
            if self.token.kind == TokenKind::Symbol(b',') {
                self.advance()?;
            }
            let value = self.pop()?;
            let key = self.pop()?;
            self.emit(Instruction {
                opcode: 0x3c,
                arg0: 0,
                arg1: i32::from(table.0),
                arg2: key.0,
                arg3: value.0,
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
}
