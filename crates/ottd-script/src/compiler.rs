use crate::{
    CompileError, CompileErrorKind, Instruction, Program, Value,
    lexer::{Lexer, Token, TokenKind},
};
mod emitter;
mod expressions;
mod registers;
mod statements;
use emitter::Emitter;
use registers::{Register, Registers};

/// Compile the supported scalar statement subset with native register allocation.
///
/// # Errors
/// Rejects unsupported syntax, malformed literals and bounded resource excess.
pub fn compile(source: &str) -> Result<Program, CompileError> {
    compile_bytes(source.as_bytes())
}
/// Compile native compilebuffer bytes with lazy UTF-8/codepoint admission.
///
/// # Errors
/// Rejects consumed invalid characters, unsupported syntax and bounded resource excess.
pub fn compile_bytes(source: &[u8]) -> Result<Program, CompileError> {
    if source.len() > 65_536 {
        return Err(CompileError {
            offset: 0,
            kind: CompileErrorKind::Limit,
        });
    }
    let mut lexer = Lexer::new(source)?;
    let token = lexer.next()?;
    let mut compiler = Compiler {
        lexer,
        token,
        previous: TokenKind::End,
        registers: Registers::new(),
        emitter: Emitter::new(),
        literals: Vec::new(),
        break_targets: Vec::new(),
        continue_targets: Vec::new(),
        last_stack_size: 0,
    };
    compiler.main()?;
    Ok(Program {
        stack_size: compiler.registers.high_water,
        literals: compiler.literals,
        instructions: compiler.emitter.instructions,
    })
}
struct Compiler<'a> {
    lexer: Lexer<'a>,
    token: Token<'a>,
    previous: TokenKind<'a>,
    registers: Registers<'a>,
    emitter: Emitter,
    literals: Vec<Value>,
    break_targets: Vec<Vec<i32>>,
    continue_targets: Vec<Vec<i32>>,
    last_stack_size: u8,
}
impl Compiler<'_> {
    const fn error(&self, kind: CompileErrorKind) -> CompileError {
        CompileError {
            offset: self.token.offset,
            kind,
        }
    }
    fn advance(&mut self) -> Result<(), CompileError> {
        let next = self.lexer.next()?;
        self.previous = if next.newline {
            TokenKind::Symbol(b'\n')
        } else {
            self.token.kind
        };
        self.token = next;
        Ok(())
    }
    fn push(&mut self) -> Result<Register, CompileError> {
        self.registers
            .push()
            .ok_or_else(|| self.error(CompileErrorKind::Limit))
    }
    fn pop(&mut self) -> Result<Register, CompileError> {
        self.registers
            .pop()
            .ok_or_else(|| self.error(CompileErrorKind::ExpectedToken))
    }
    fn size(&self) -> Result<u8, CompileError> {
        self.registers
            .size()
            .ok_or_else(|| self.error(CompileErrorKind::Limit))
    }
    fn position(&self) -> Result<i32, CompileError> {
        self.emitter
            .position()
            .ok_or_else(|| self.error(CompileErrorKind::Limit))
    }
    fn patch(&mut self, position: i32, target: i32) -> Result<(), CompileError> {
        let offset = target
            .checked_sub(position)
            .ok_or_else(|| self.error(CompileErrorKind::Limit))?;
        self.emitter
            .patch(position, offset)
            .ok_or_else(|| self.error(CompileErrorKind::Limit))
    }
    fn emit(&mut self, instruction: Instruction) {
        self.emitter.emit(instruction, &self.registers);
    }
    fn expect(&mut self, symbol: u8) -> Result<(), CompileError> {
        if self.token.kind != TokenKind::Symbol(symbol) {
            return Err(self.error(CompileErrorKind::ExpectedToken));
        }
        self.advance()
    }
    const fn depth(&self, depth: u8) -> Result<u8, CompileError> {
        if depth >= 128 {
            return Err(self.error(CompileErrorKind::Limit));
        }
        Ok(depth.saturating_add(1))
    }
    fn load(&mut self, register: Register, value: Value) -> Result<(), CompileError> {
        let (opcode, arg1) = match value {
            Value::Integer(n) if (0..=i64::from(i32::MAX)).contains(&n) => (
                0x02,
                i32::try_from(n).map_err(|_| self.error(CompileErrorKind::Limit))?,
            ),
            Value::Integer(_) => {
                let index = self
                    .literals
                    .iter()
                    .position(|item| *item == value)
                    .unwrap_or_else(|| {
                        self.literals.push(value);
                        self.literals.len().saturating_sub(1)
                    });
                (
                    0x01,
                    i32::try_from(index).map_err(|_| self.error(CompileErrorKind::Limit))?,
                )
            }
            Value::Float(bits) => (0x03, i32::from_ne_bytes(bits.to_ne_bytes())),
            Value::Null => (0x14, 1),
            Value::Bool(b) => (0x16, i32::from(b)),
        };
        self.emit(Instruction {
            opcode,
            arg0: register.0,
            arg1,
            arg2: 0,
            arg3: 0,
        });
        Ok(())
    }
}
