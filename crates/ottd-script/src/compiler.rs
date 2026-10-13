use crate::compiler::diagnostics::CompileFailure;
use crate::{
    CompileErrorKind, Instruction, Program, Value,
    lexer::{Lexer, Token, TokenKind},
};
#[expect(
    clippy::redundant_pub_crate,
    reason = "Private failure transport is shared with the lexer and runner"
)]
pub(crate) mod diagnostics;
mod emitter;
mod expressions;
mod functions;
mod registers;
mod statements;
use emitter::Emitter;
use registers::{Register, Registers};

/// Compile the supported scalar statement subset with native register allocation.
///
/// # Errors
/// Rejects unsupported syntax, malformed literals and bounded resource excess.
pub fn compile(source: &str) -> Result<Program, crate::CompileError> {
    compile_bytes(source.as_bytes())
}
/// Compile bytes with the native compilebuffer decoder and lazy NUL termination.
///
/// # Errors
/// Rejects consumed invalid characters, reached undefined native ctype arguments,
/// unsupported syntax and bounded resource excess.
pub fn compile_bytes(source: &[u8]) -> Result<Program, crate::CompileError> {
    crate::Realm::new().compile_bytes(source)
}
#[expect(
    clippy::redundant_pub_crate,
    reason = "Realm compilation must remain crate-private while public wrappers are reexported"
)]
pub(crate) fn compile_in(
    realm: &crate::Realm,
    source: &[u8],
    source_name: &[u8],
) -> Result<Program, CompileFailure> {
    if source.len() > 65_536 {
        return Err(CompileFailure::new(0, CompileErrorKind::Limit));
    }
    let mut lexer = Lexer::new(source, realm.clone())?;
    let token = lexer.next()?;
    let mut compiler = Compiler {
        realm: realm.clone(),
        source_name: realm.string(source_name),
        lexer,
        token,
        previous: TokenKind::End,
        registers: Registers::new(),
        emitter: Emitter::new(),
        literals: Vec::new(),
        break_targets: Vec::new(),
        continue_targets: Vec::new(),
        last_stack_size: 0,
        traps: 0,
        captures: Vec::new(),
        children: Vec::new(),
        function_depth: 0,
    };
    compiler.main()?;
    compiler.registers.truncate(0);
    let data = crate::prototype::Data {
        source_name: Some(compiler.source_name.clone()),
        name: Some(realm.string(b"main")),
        parameter_names: vec![realm.string(b"this")],
        capture_names: Vec::new(),
        local_names: compiler
            .registers
            .local_names
            .iter()
            .map(|name| realm.string(name.as_bytes()))
            .collect(),
        stack_size: compiler.registers.high_water,
        literals: compiler.scalar_literals()?,
        instructions: compiler.emitter.instructions,
        children: compiler.children,
        captures: Vec::new(),
        defaults: Vec::new(),
        parameters: 1,
        variadic: false,
    };
    crate::ScriptClosure::new(realm, std::rc::Rc::new(data))
        .map(|data| Program { data })
        .map_err(|_| CompileFailure::new(source.len(), CompileErrorKind::Limit))
}
struct Compiler<'a> {
    realm: crate::Realm,
    source_name: crate::ByteString,
    lexer: Lexer<'a>,
    token: Token<'a>,
    previous: TokenKind<'a>,
    registers: Registers<'a>,
    emitter: Emitter,
    literals: Vec<Value>,
    break_targets: Vec<statements::ExitTarget>,
    continue_targets: Vec<statements::ExitTarget>,
    last_stack_size: u8,
    traps: u8,
    captures: Vec<(&'a str, crate::prototype::Capture)>,
    children: Vec<std::rc::Rc<crate::prototype::Data>>,
    function_depth: u8,
}
impl Compiler<'_> {
    const fn error(&self, kind: CompileErrorKind) -> CompileFailure {
        CompileFailure::new(self.token.offset, kind)
    }
    fn expected_identifier(&self) -> CompileFailure {
        self.error(CompileErrorKind::ExpectedToken)
            .native("expected 'IDENTIFIER'")
    }
    fn advance(&mut self) -> Result<(), CompileFailure> {
        let next = self.lexer.next()?;
        self.previous = if next.newline {
            TokenKind::Symbol(b'\n')
        } else {
            self.token.kind.clone()
        };
        self.token = next;
        Ok(())
    }
    fn push(&mut self) -> Result<Register, CompileFailure> {
        self.registers
            .push()
            .ok_or_else(|| self.error(CompileErrorKind::Limit))
    }
    fn pop(&mut self) -> Result<Register, CompileFailure> {
        self.registers
            .pop()
            .ok_or_else(|| self.error(CompileErrorKind::ExpectedToken))
    }
    fn size(&self) -> Result<u8, CompileFailure> {
        self.registers
            .size()
            .ok_or_else(|| self.error(CompileErrorKind::Limit))
    }
    fn position(&self) -> Result<i32, CompileFailure> {
        self.emitter
            .position()
            .ok_or_else(|| self.error(CompileErrorKind::Limit))
    }
    fn patch(&mut self, position: i32, target: i32) -> Result<(), CompileFailure> {
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
    fn expect(&mut self, symbol: u8) -> Result<(), CompileFailure> {
        if self.token.kind != TokenKind::Symbol(symbol) {
            return Err(self.error(CompileErrorKind::ExpectedToken).expected(symbol));
        }
        self.advance()
    }
    const fn depth(&self, depth: u8) -> Result<u8, CompileFailure> {
        if depth >= 128 {
            return Err(self.error(CompileErrorKind::Limit));
        }
        Ok(depth.saturating_add(1))
    }
    fn load(&mut self, register: Register, value: Value) -> Result<(), CompileFailure> {
        let (opcode, arg1) = match value {
            Value::Integer(n) if (0..=i64::from(i32::MAX)).contains(&n) => (
                0x02,
                i32::try_from(n).map_err(|_| self.error(CompileErrorKind::Limit))?,
            ),
            Value::Integer(_) | Value::String(_) => {
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
            Value::Array(_)
            | Value::Table(_)
            | Value::NativeClosure(_)
            | Value::ScriptClosure(_) => {
                return Err(self.error(CompileErrorKind::UnsupportedSyntax));
            }
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
