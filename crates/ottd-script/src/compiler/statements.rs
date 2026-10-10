//! Statement boundaries, declarations and scope lifetime.
use super::Compiler;
use crate::{CompileError, CompileErrorKind, Instruction, Value, lexer::TokenKind};
mod branches;
mod iteration;
impl Compiler<'_> {
    pub(super) fn main(&mut self) -> Result<(), CompileError> {
        while self.token.kind != TokenKind::End {
            self.statement(0)?;
            if self.previous != TokenKind::Symbol(b'}') {
                self.semicolon()?;
            }
        }
        self.registers.truncate(1);
        self.emit(Instruction {
            opcode: 0x13,
            arg0: 255,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        Ok(())
    }
    const fn end_statement(&self) -> bool {
        self.token.newline
            || matches!(
                self.token.kind,
                TokenKind::End | TokenKind::Symbol(b';' | b'}')
            )
    }
    fn semicolon(&mut self) -> Result<(), CompileError> {
        if self.token.kind == TokenKind::Symbol(b';') {
            return self.advance();
        }
        if !self.end_statement() {
            return Err(self.error(CompileErrorKind::ExpectedToken));
        }
        Ok(())
    }
    fn statement(&mut self, depth: u8) -> Result<(), CompileError> {
        let depth = self.depth(depth)?;
        match self.token.kind {
            TokenKind::Symbol(b';') => {
                self.advance()?;
            }
            TokenKind::Symbol(b'{') => {
                self.block(depth)?;
            }
            TokenKind::Return => {
                self.return_statement()?;
            }
            TokenKind::Local => {
                self.local()?;
            }
            TokenKind::If => self.conditional(depth)?,
            TokenKind::While => self.while_loop(depth)?,
            TokenKind::For => self.for_loop(depth)?,
            TokenKind::Do => self.do_loop(depth)?,
            TokenKind::Break => {
                self.loop_exit(false)?;
            }
            TokenKind::Continue => {
                self.loop_exit(true)?;
            }
            TokenKind::Compound(_)
            | TokenKind::Increment(_)
            | TokenKind::Shift(_)
            | TokenKind::Identifier(_)
            | TokenKind::Scalar(_)
            | TokenKind::Symbol(_)
            | TokenKind::Equal
            | TokenKind::NotEqual
            | TokenKind::LessEqual
            | TokenKind::GreaterEqual
            | TokenKind::And
            | TokenKind::Or
            | TokenKind::Else
            | TokenKind::End => {
                self.comma(0)?;
                let _target = self.pop()?;
            }
        }
        self.emitter.barrier();
        Ok(())
    }
    fn return_statement(&mut self) -> Result<(), CompileError> {
        self.advance()?;
        let (arg0, arg1) = if self.end_statement() {
            (255, 0)
        } else {
            self.comma(0)?;
            (1, i32::from(self.pop()?.0))
        };
        self.emit(Instruction {
            opcode: 0x13,
            arg0,
            arg1,
            arg2: 0,
            arg3: 0,
        });
        Ok(())
    }
    fn block(&mut self, depth: u8) -> Result<(), CompileError> {
        let start = self.size()?;
        self.advance()?;
        while self.token.kind != TokenKind::Symbol(b'}') {
            self.statement(depth)?;
            if !matches!(self.previous, TokenKind::Symbol(b'}' | b';')) {
                self.semicolon()?;
            }
        }
        self.expect(b'}')?;
        self.scope_end(start)?;
        self.registers.truncate(start);
        Ok(())
    }
    fn local(&mut self) -> Result<(), CompileError> {
        loop {
            self.advance()?;
            let TokenKind::Identifier(name) = self.token.kind else {
                return Err(self.error(CompileErrorKind::ExpectedToken));
            };
            self.advance()?;
            if self.token.kind == TokenKind::Symbol(b'=') {
                self.advance()?;
                let _local = self.expression(0)?;
                let source = self.pop()?;
                let target = self.push()?;
                if target != source {
                    self.move_to(target, source);
                }
            } else {
                let target = self.push()?;
                self.load(target, Value::Null)?;
            }
            let _target = self.pop()?;
            let _local = self
                .registers
                .bind(name)
                .ok_or_else(|| self.error(CompileErrorKind::Limit))?;
            if self.token.kind != TokenKind::Symbol(b',') {
                break;
            }
        }
        Ok(())
    }
    fn scope_end(&mut self, start: u8) -> Result<(), CompileError> {
        self.emit(Instruction {
            opcode: 0x3d,
            arg0: start,
            arg1: i32::from(self.size()?),
            arg2: 0,
            arg3: 0,
        });
        Ok(())
    }
}
