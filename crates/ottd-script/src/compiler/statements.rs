//! Statement boundaries, declarations and scope lifetime.
use super::Compiler;
use crate::compiler::diagnostics::CompileFailure;
use crate::{CompileErrorKind, Instruction, Value, lexer::TokenKind};
mod branches;
mod constants;
mod foreach;
mod iteration;
mod switch;
mod traps;
#[derive(Default)]
pub(super) struct ExitTarget {
    jumps: Vec<i32>,
    traps: u8,
}
impl Compiler<'_> {
    pub(super) fn main(&mut self) -> Result<(), CompileFailure> {
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
    fn semicolon(&mut self) -> Result<(), CompileFailure> {
        if self.token.kind == TokenKind::Symbol(b';') {
            return self.advance();
        }
        if !self.end_statement() {
            return Err(self
                .error(CompileErrorKind::ExpectedToken)
                .native("end of statement expected (; or lf)"));
        }
        Ok(())
    }
    pub(super) fn statement(&mut self, depth: u8) -> Result<(), CompileFailure> {
        let depth = self.depth(depth)?;
        match self.token.kind.clone() {
            TokenKind::Symbol(b';') => {
                self.advance()?;
            }
            TokenKind::Symbol(b'{') => {
                self.block(depth)?;
            }
            TokenKind::Return => {
                self.return_statement()?;
            }
            TokenKind::Try => self.try_catch(depth)?,
            TokenKind::Throw => self.throw_statement()?,
            TokenKind::Function => self.function_statement(depth)?,
            TokenKind::Const => self.constant_declaration()?,
            TokenKind::Enum => self.enum_declaration()?,
            TokenKind::Local => {
                self.local()?;
            }
            TokenKind::If => self.conditional(depth)?,
            TokenKind::Switch => self.switch_statement(depth)?,
            TokenKind::While => self.while_loop(depth)?,
            TokenKind::For => self.for_loop(depth)?,
            TokenKind::Foreach => self.foreach_loop(depth)?,
            TokenKind::Do => self.do_loop(depth)?,
            TokenKind::Break => {
                self.loop_exit(false)?;
            }
            TokenKind::Continue => {
                self.loop_exit(true)?;
            }
            TokenKind::Delegate
            | TokenKind::Parent
            | TokenKind::Catch
            | TokenKind::In
            | TokenKind::Ellipsis
            | TokenKind::Vargc
            | TokenKind::Vargv
            | TokenKind::This
            | TokenKind::Root
            | TokenKind::NewSlot
            | TokenKind::Clone
            | TokenKind::Delete
            | TokenKind::TypeOf
            | TokenKind::Case
            | TokenKind::Default
            | TokenKind::Compound(_)
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
    fn return_statement(&mut self) -> Result<(), CompileFailure> {
        self.advance()?;
        let return_start = self.emitter.instructions.len();
        let (arg0, arg1) = if self.end_statement() {
            (255, 0)
        } else {
            self.comma(0)?;
            (1, i32::from(self.pop()?.0))
        };
        self.pop_traps(self.traps);
        if self.function_depth > 0 && arg0 != 255 && self.emitter.instructions.len() > return_start
        {
            if let Some(last) = self.emitter.instructions.last_mut() {
                if last.opcode == 0x06 {
                    last.opcode = 0x05;
                }
            }
        }
        self.emit(Instruction {
            opcode: 0x13,
            arg0,
            arg1,
            arg2: 0,
            arg3: 0,
        });
        Ok(())
    }
    fn block(&mut self, depth: u8) -> Result<(), CompileFailure> {
        let start = self.size()?;
        self.advance()?;
        self.statements(depth)?;
        self.expect(b'}')?;
        self.scope_end(start)?;
        self.registers.truncate(start);
        Ok(())
    }
    fn statements(&mut self, depth: u8) -> Result<(), CompileFailure> {
        while !matches!(
            self.token.kind,
            TokenKind::Symbol(b'}') | TokenKind::Case | TokenKind::Default
        ) {
            self.statement(depth)?;
            if !matches!(self.previous, TokenKind::Symbol(b'}' | b';')) {
                self.semicolon()?;
            }
        }
        Ok(())
    }
    fn local(&mut self) -> Result<(), CompileFailure> {
        loop {
            self.advance()?;
            let TokenKind::Identifier(name) = self.token.kind.clone() else {
                return Err(self.expected_identifier());
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
    fn scope_end(&mut self, start: u8) -> Result<(), CompileFailure> {
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
