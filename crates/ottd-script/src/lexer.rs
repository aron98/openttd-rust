#![expect(
    clippy::redundant_pub_crate,
    reason = "Crate-private lexer types must satisfy unreachable_pub"
)]
use crate::{CompileError, CompileErrorKind, NativeCharacterContext, Value};
mod comments;
mod input;
mod numbers;
mod strings;
mod token;
pub(crate) use token::{Token, TokenKind};
pub(crate) struct Lexer<'a> {
    source: &'a [u8],
    realm: crate::Realm,
    width: usize,
    character: u32,
    position: usize,
}
impl<'a> Lexer<'a> {
    pub(super) fn new(source: &'a [u8], realm: crate::Realm) -> Result<Self, CompileError> {
        let mut lexer = Self {
            source,
            realm,
            width: 0,
            character: 0,
            position: 0,
        };
        lexer.read()?;
        Ok(lexer)
    }
    fn peek(&self) -> Option<u8> {
        if self.width == 0 {
            None
        } else {
            self.source.get(self.position).copied()
        }
    }
    fn advance(&mut self) -> Result<(), CompileError> {
        if self.width == 0 {
            return Ok(());
        }
        self.position = self.position.saturating_add(self.width);
        self.read()
    }
    const fn error(&self, kind: CompileErrorKind) -> CompileError {
        CompileError {
            offset: self.position,
            kind,
        }
    }
    pub(super) fn next(&mut self) -> Result<Token<'a>, CompileError> {
        let newline = self.trivia()?;
        let offset = self.position;
        let Some(c) = self.peek() else {
            return Ok(Token {
                kind: TokenKind::End,
                offset,
                newline,
            });
        };
        self.classify(NativeCharacterContext::Token)?;
        let kind = match c {
            b'"' | b'\'' => TokenKind::Scalar(self.string(false)?),
            b'@' => {
                self.advance()?;
                if self.peek() != Some(b'"') {
                    return Err(self.error(CompileErrorKind::ExpectedToken));
                }
                TokenKind::Scalar(self.string(true)?)
            }
            b'0'..=b'9' => TokenKind::Scalar(self.number()?),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                while self
                    .classify(NativeCharacterContext::Identifier)?
                    .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
                {
                    self.advance()?;
                }
                let name = self
                    .source
                    .get(offset..self.position)
                    .ok_or_else(|| self.error(CompileErrorKind::UnsupportedSyntax))?;
                let name = std::str::from_utf8(name)
                    .map_err(|_| self.error(CompileErrorKind::InvalidCharacter))?;
                match name {
                    "const" => TokenKind::Const,
                    "enum" => TokenKind::Enum,
                    "typeof" => TokenKind::TypeOf,
                    "return" => TokenKind::Return,
                    "local" => TokenKind::Local,
                    "if" => TokenKind::If,
                    "else" => TokenKind::Else,
                    "while" => TokenKind::While,
                    "switch" => TokenKind::Switch,
                    "case" => TokenKind::Case,
                    "default" => TokenKind::Default,
                    "for" => TokenKind::For,
                    "do" => TokenKind::Do,
                    "break" => TokenKind::Break,
                    "continue" => TokenKind::Continue,
                    "null" => TokenKind::Scalar(Value::Null),
                    "true" => TokenKind::Scalar(Value::Bool(true)),
                    "false" => TokenKind::Scalar(Value::Bool(false)),
                    "function" | "foreach" | "in" | "delegate" | "delete" | "try" | "catch"
                    | "throw" | "clone" | "yield" | "resume" | "this" | "parent" | "class"
                    | "extends" | "instanceof" | "vargc" | "vargv" | "static" => {
                        return Err(CompileError {
                            offset,
                            kind: CompileErrorKind::UnsupportedSyntax,
                        });
                    }
                    _ => TokenKind::Identifier(name),
                }
            }
            b'+' | b'-' | b'*' | b'/' | b'%' | b'!' | b'~' | b'(' | b')' | b';' | b'{' | b'}'
            | b',' | b'?' | b':' | b'^' | b'=' | b'<' | b'>' | b'&' | b'|' | b'.' => {
                self.symbol(c)?
            }
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
        self.advance()?;
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
                self.advance()?;
                if self.peek() == Some(b'>') {
                    self.advance()?;
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
            self.advance()?;
            Ok(token)
        })
    }
}
