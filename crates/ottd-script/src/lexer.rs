#![expect(
    clippy::redundant_pub_crate,
    reason = "Crate-private lexer types must also satisfy workspace unreachable_pub"
)]

use crate::{CompileError, CompileErrorKind, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Return,
    Scalar(Value),
    Symbol(u8),
    End,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Token {
    pub kind: TokenKind,
    pub offset: usize,
    pub newline: bool,
}
pub(crate) struct Lexer<'a> {
    source: &'a str,
    position: usize,
}
impl<'a> Lexer<'a> {
    pub(super) const fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
        }
    }
    fn peek(&self) -> Option<u8> {
        self.source.as_bytes().get(self.position).copied()
    }
    const fn advance(&mut self) {
        self.position = self.position.saturating_add(1);
    }
    const fn error(&self, kind: CompileErrorKind) -> CompileError {
        CompileError {
            offset: self.position,
            kind,
        }
    }
    pub(super) fn next(&mut self) -> Result<Token, CompileError> {
        let mut newline = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() {
                newline |= c == b'\n';
                self.advance();
            } else {
                break;
            }
        }
        let offset = self.position;
        let Some(c) = self.peek() else {
            return Ok(Token {
                kind: TokenKind::End,
                offset,
                newline,
            });
        };
        let kind = match c {
            b'0'..=b'9' => TokenKind::Scalar(self.number()?),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                while self
                    .peek()
                    .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
                {
                    self.advance();
                }
                match self.source.get(offset..self.position) {
                    Some("return") => TokenKind::Return,
                    Some("null") => TokenKind::Scalar(Value::Null),
                    Some("true") => TokenKind::Scalar(Value::Bool(true)),
                    Some("false") => TokenKind::Scalar(Value::Bool(false)),
                    _ => {
                        return Err(CompileError {
                            offset,
                            kind: CompileErrorKind::UnsupportedSyntax,
                        });
                    }
                }
            }
            b'+' | b'-' | b'*' | b'/' | b'%' | b'!' | b'~' | b'(' | b')' | b';' => {
                self.advance();
                if matches!(c, b'+' | b'-') && self.peek() == Some(c) {
                    return Err(CompileError {
                        offset,
                        kind: CompileErrorKind::UnsupportedSyntax,
                    });
                }
                TokenKind::Symbol(c)
            }
            _ => return Err(self.error(CompileErrorKind::UnsupportedSyntax)),
        };
        Ok(Token {
            kind,
            offset,
            newline,
        })
    }
    fn number(&mut self) -> Result<Value, CompileError> {
        let start = self.position;
        let first = self.peek();
        self.advance();
        let radix = if first == Some(b'0') {
            match self.peek() {
                Some(b'x' | b'X') => {
                    self.advance();
                    Some(16)
                }
                Some(b'0'..=b'7') => Some(8),
                _ => None,
            }
        } else {
            None
        };
        if let Some(radix) = radix {
            let digits = self.position;
            while self.peek().is_some_and(|b| {
                if radix == 16 {
                    b.is_ascii_hexdigit()
                } else {
                    (b'0'..=b'7').contains(&b)
                }
            }) {
                self.advance();
            }
            if (radix == 8 && self.peek().is_some_and(|b| b.is_ascii_digit()))
                || (radix == 16 && self.position.saturating_sub(digits) > 16)
            {
                return Err(self.error(CompileErrorKind::InvalidNumber));
            }
            let text = self
                .source
                .get(digits..self.position)
                .ok_or_else(|| self.error(CompileErrorKind::InvalidNumber))?;
            return Ok(integer(text, radix));
        }
        let mut float = false;
        while let Some(c) = self.peek() {
            match c {
                b'0'..=b'9' => self.advance(),
                b'.' => {
                    float = true;
                    self.advance();
                }
                b'e' | b'E' => {
                    float = true;
                    self.advance();
                    if matches!(self.peek(), Some(b'+' | b'-')) {
                        self.advance();
                    }
                    if !self.peek().is_some_and(|b| b.is_ascii_digit()) {
                        return Err(self.error(CompileErrorKind::InvalidNumber));
                    }
                }
                _ => break,
            }
        }
        let text = self
            .source
            .get(start..self.position)
            .ok_or_else(|| self.error(CompileErrorKind::InvalidNumber))?;
        if float {
            parse_float(text)
                .map(Value::Float)
                .ok_or_else(|| self.error(CompileErrorKind::InvalidNumber))
        } else {
            Ok(integer(text, 10))
        }
    }
}
fn integer(text: &str, radix: u32) -> Value {
    let unsigned = u64::from_str_radix(text, radix).unwrap_or(0);
    Value::Integer(i64::from_ne_bytes(unsigned.to_ne_bytes()))
}
#[expect(
    clippy::cast_possible_truncation,
    reason = "Native lexing uses strtod followed by an explicit SQFloat=f32 cast"
)]
fn parse_float(text: &str) -> Option<u32> {
    text.parse::<f64>().ok().map(|n| (n as f32).to_bits())
}
