use crate::{
    CompileError, CompileErrorKind, Instruction, Program, Value,
    lexer::{Lexer, Token, TokenKind},
};

/// Compile the supported scalar-return subset with native register allocation.
///
/// # Errors
/// Rejects unsupported syntax, malformed literals and bounded resource excess.
pub fn compile(source: &str) -> Result<Program, CompileError> {
    if source.len() > 65_536 {
        return Err(CompileError {
            offset: 0,
            kind: CompileErrorKind::Limit,
        });
    }
    let mut lexer = Lexer::new(source);
    let token = lexer.next()?;
    let mut compiler = Compiler {
        lexer,
        token,
        program: Program {
            stack_size: 1,
            literals: Vec::new(),
            instructions: Vec::new(),
        },
        next_register: 1,
    };
    compiler.main()?;
    Ok(compiler.program)
}
struct Compiler<'a> {
    lexer: Lexer<'a>,
    token: Token,
    program: Program,
    next_register: u8,
}
impl Compiler<'_> {
    const fn error(&self, kind: CompileErrorKind) -> CompileError {
        CompileError {
            offset: self.token.offset,
            kind,
        }
    }
    fn advance(&mut self) -> Result<(), CompileError> {
        self.token = self.lexer.next()?;
        Ok(())
    }
    fn main(&mut self) -> Result<(), CompileError> {
        if self.token.kind != TokenKind::Return {
            return Err(self.error(CompileErrorKind::UnsupportedSyntax));
        }
        self.advance()?;
        if self.token.newline || matches!(self.token.kind, TokenKind::End | TokenKind::Symbol(b';'))
        {
            self.emit(Instruction {
                opcode: 0x13,
                arg0: 255,
                arg1: 0,
                arg2: 0,
                arg3: 0,
            });
        } else {
            let reg = self.expression(0, 0)?;
            self.emit(Instruction {
                opcode: 0x13,
                arg0: 1,
                arg1: i32::from(reg),
                arg2: 0,
                arg3: 0,
            });
        }
        if self.token.kind == TokenKind::Symbol(b';') {
            self.advance()?;
        }
        if self.token.kind != TokenKind::End {
            return Err(self.error(CompileErrorKind::UnsupportedSyntax));
        }
        self.emit(Instruction {
            opcode: 0x13,
            arg0: 255,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        Ok(())
    }
    fn expression(&mut self, precedence: u8, depth: u8) -> Result<u8, CompileError> {
        if depth >= 128 {
            return Err(self.error(CompileErrorKind::Limit));
        }
        let left = self.factor(depth.saturating_add(1))?;
        while let TokenKind::Symbol(op) = self.token.kind {
            let rank = match op {
                b'+' | b'-' => 1,
                b'*' | b'/' | b'%' => 2,
                _ => break,
            };
            if rank <= precedence {
                break;
            }
            self.advance()?;
            let right = self.expression(rank, depth.saturating_add(1))?;
            self.next_register = left.saturating_add(1);
            self.emit(Instruction {
                opcode: 0x11,
                arg0: left,
                arg1: i32::from(right),
                arg2: left,
                arg3: op,
            });
        }
        Ok(left)
    }
    fn factor(&mut self, depth: u8) -> Result<u8, CompileError> {
        if depth >= 128 {
            return Err(self.error(CompileErrorKind::Limit));
        }
        match self.token.kind {
            TokenKind::Scalar(value) => {
                self.advance()?;
                let reg = self.allocate()?;
                self.load(reg, value)?;
                Ok(reg)
            }
            TokenKind::Symbol(b'(') => {
                self.advance()?;
                let reg = self.expression(0, depth.saturating_add(1))?;
                if self.token.kind != TokenKind::Symbol(b')') {
                    return Err(self.error(CompileErrorKind::ExpectedToken));
                }
                self.advance()?;
                Ok(reg)
            }
            TokenKind::Symbol(op @ (b'-' | b'!' | b'~')) => {
                self.advance()?;
                let reg = self.factor(depth.saturating_add(1))?;
                let opcode = match op {
                    b'-' => 0x2d,
                    b'!' => 0x2e,
                    _ => 0x2f,
                };
                self.emit(Instruction {
                    opcode,
                    arg0: reg,
                    arg1: i32::from(reg),
                    arg2: 0,
                    arg3: 0,
                });
                Ok(reg)
            }
            TokenKind::Symbol(_) | TokenKind::Return | TokenKind::End => {
                Err(self.error(CompileErrorKind::ExpectedToken))
            }
        }
    }
    fn allocate(&mut self) -> Result<u8, CompileError> {
        let reg = self.next_register;
        self.next_register = reg
            .checked_add(1)
            .ok_or_else(|| self.error(CompileErrorKind::Limit))?;
        self.program.stack_size = self.program.stack_size.max(u16::from(self.next_register));
        Ok(reg)
    }
    fn load(&mut self, reg: u8, value: Value) -> Result<(), CompileError> {
        let (opcode, arg1) = match value {
            Value::Integer(n) if (0..=i64::from(i32::MAX)).contains(&n) => (
                0x02,
                i32::try_from(n).map_err(|_| self.error(CompileErrorKind::Limit))?,
            ),
            Value::Integer(_) => {
                let index = self
                    .program
                    .literals
                    .iter()
                    .position(|item| *item == value)
                    .unwrap_or_else(|| {
                        self.program.literals.push(value);
                        self.program.literals.len().saturating_sub(1)
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
            arg0: reg,
            arg1,
            arg2: 0,
            arg3: 0,
        });
        Ok(())
    }
    fn emit(&mut self, instruction: Instruction) {
        if let Some(previous) = self.program.instructions.last_mut() {
            if previous.opcode == 0x14
                && instruction.opcode == 0x14
                && i32::from(previous.arg0).checked_add(previous.arg1)
                    == Some(i32::from(instruction.arg0))
            {
                if let Some(count) = previous.arg1.checked_add(instruction.arg1) {
                    previous.arg1 = count;
                    return;
                }
            }
            if previous.opcode == 0x01 && instruction.opcode == 0x01 {
                if let Ok(index) = u8::try_from(instruction.arg1) {
                    previous.opcode = 0x04;
                    previous.arg2 = instruction.arg0;
                    previous.arg3 = index;
                    return;
                }
            }
        }
        self.program.instructions.push(instruction);
    }
}
