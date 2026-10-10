#![expect(
    clippy::redundant_pub_crate,
    reason = "Crate-private lexer types must satisfy unreachable_pub"
)]
use crate::{CompileError, CompileErrorKind, Value};
mod numbers;
mod token;
pub(crate) use token::{Token, TokenKind};
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
    pub(super) fn next(&mut self) -> Result<Token<'a>, CompileError> {
        let mut newline = false;
        while let Some(c) = self.peek() {
            if matches!(c, b' ' | b'\t' | b'\r' | b'\n') {
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
                let name = self
                    .source
                    .get(offset..self.position)
                    .ok_or_else(|| self.error(CompileErrorKind::UnsupportedSyntax))?;
                match name {
                    "return" => TokenKind::Return,
                    "local" => TokenKind::Local,
                    "if" => TokenKind::If,
                    "else" => TokenKind::Else,
                    "while" => TokenKind::While,
                    "for" => TokenKind::For,
                    "do" => TokenKind::Do,
                    "break" => TokenKind::Break,
                    "continue" => TokenKind::Continue,
                    "null" => TokenKind::Scalar(Value::Null),
                    "true" => TokenKind::Scalar(Value::Bool(true)),
                    "false" => TokenKind::Scalar(Value::Bool(false)),
                    "function" | "foreach" | "in" | "typeof" | "delegate" | "delete" | "try"
                    | "catch" | "throw" | "clone" | "yield" | "resume" | "switch" | "case"
                    | "default" | "this" | "parent" | "class" | "extends" | "constructor"
                    | "instanceof" | "vargc" | "vargv" | "static" | "enum" | "const" => {
                        return Err(CompileError {
                            offset,
                            kind: CompileErrorKind::UnsupportedSyntax,
                        });
                    }
                    _ => TokenKind::Identifier(name),
                }
            }
            b'+' | b'-' | b'*' | b'/' | b'%' | b'!' | b'~' | b'(' | b')' | b';' | b'{' | b'}'
            | b',' | b'?' | b':' | b'^' | b'=' | b'<' | b'>' | b'&' | b'|' => self.symbol(c)?,
            _ => return Err(self.error(CompileErrorKind::UnsupportedSyntax)),
        };
        Ok(Token {
            kind,
            offset,
            newline,
        })
    }
    fn symbol(&mut self, first: u8) -> Result<TokenKind<'a>, CompileError> {
        let offset = self.position;
        self.advance();
        let pair = (first, self.peek());
        let combined = match pair {
            (b'=', Some(b'=')) => Some(TokenKind::Equal),
            (b'!', Some(b'=')) => Some(TokenKind::NotEqual),
            (b'<', Some(b'=')) => Some(TokenKind::LessEqual),
            (b'>', Some(b'=')) => Some(TokenKind::GreaterEqual),
            (b'&', Some(b'&')) => Some(TokenKind::And),
            (b'|', Some(b'|')) => Some(TokenKind::Or),
            (b'+', Some(b'+')) => Some(TokenKind::Increment(1)),
            (b'-', Some(b'-')) => Some(TokenKind::Increment(-1)),
            (operator @ (b'+' | b'-' | b'*' | b'/' | b'%'), Some(b'=')) => {
                Some(TokenKind::Compound(operator))
            }
            (b'<', Some(b'<')) => Some(TokenKind::Shift(4)),
            (b'>', Some(b'>')) => {
                self.advance();
                if self.peek() == Some(b'>') {
                    self.advance();
                    return Ok(TokenKind::Shift(6));
                }
                return Ok(TokenKind::Shift(5));
            }
            (b'/', Some(b'/' | b'*' | b'>')) | (b'<', Some(b'-' | b'/')) | (b':', Some(b':')) => {
                return Err(CompileError {
                    offset,
                    kind: CompileErrorKind::UnsupportedSyntax,
                });
            }
            _ => None,
        };
        combined.map_or(Ok(TokenKind::Symbol(first)), |token| {
            self.advance();
            Ok(token)
        })
    }
}
