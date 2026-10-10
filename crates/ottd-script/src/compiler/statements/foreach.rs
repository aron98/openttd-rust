//! Native foreach retains its iterable target and three adjacent local registers.
use super::super::Compiler;
use crate::{CompileError, CompileErrorKind, Instruction, Value, lexer::TokenKind};

impl Compiler<'_> {
    pub(super) fn foreach_loop(&mut self, depth: u8) -> Result<(), CompileError> {
        self.advance()?;
        self.expect(b'(')?;
        let TokenKind::Identifier(first) = self.token.kind else {
            return Err(self.error(CompileErrorKind::ExpectedToken));
        };
        self.advance()?;
        let (index, value) = if self.token.kind == TokenKind::Symbol(b',') {
            self.advance()?;
            let TokenKind::Identifier(value) = self.token.kind else {
                return Err(self.error(CompileErrorKind::ExpectedToken));
            };
            self.advance()?;
            (first, value)
        } else {
            ("@INDEX@", first)
        };
        if self.token.kind != TokenKind::In {
            return Err(self.error(CompileErrorKind::ExpectedToken));
        }
        self.advance()?;
        let size = self.size()?;
        let _expression = self.expression(0)?;
        self.expect(b')')?;
        let container = self
            .registers
            .top()
            .ok_or_else(|| self.error(CompileErrorKind::ExpectedToken))?;
        let mut index_position = None;
        for name in [index, value, "@ITERATOR@"] {
            let register = self
                .registers
                .bind(name)
                .ok_or_else(|| self.error(CompileErrorKind::Limit))?;
            let _first = index_position.get_or_insert(register);
            self.load(register, Value::Null)?;
        }
        let index = index_position.ok_or_else(|| self.error(CompileErrorKind::Limit))?;
        let head = self.position()?;
        for opcode in [0x33, 0x34] {
            self.emit(Instruction {
                opcode,
                arg0: container.0,
                arg1: 0,
                arg2: index.0,
                arg3: 0,
            });
        }
        self.begin_loop()?;
        self.statement(depth)?;
        self.emit(Instruction {
            opcode: 0x18,
            arg0: 0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        self.patch(self.position()?, head)?;
        let end = self.position()?;
        for (offset, adjustment) in [(1, 0), (2, 1)] {
            let branch = head
                .checked_add(offset)
                .ok_or_else(|| self.error(CompileErrorKind::Limit))?;
            let target = end
                .checked_add(adjustment)
                .ok_or_else(|| self.error(CompileErrorKind::Limit))?;
            self.patch(branch, target)?;
        }
        self.registers.truncate(size);
        self.finish_loop(head)
    }
}
