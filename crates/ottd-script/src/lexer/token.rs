//! Compiler tokens; unsupported reserved words are rejected during lexing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind<'a> {
    Return,
    Local,
    If,
    Else,
    While,
    Break,
    Continue,
    Identifier(&'a str),
    Scalar(crate::Value),
    Symbol(u8),
    Equal,
    NotEqual,
    LessEqual,
    GreaterEqual,
    And,
    Or,
    End,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub offset: usize,
    pub newline: bool,
}
impl TokenKind<'_> {
    pub(crate) const fn binary(self) -> Option<(u8, u8, u8)> {
        match self {
            Self::Equal => Some((1, 0x0f, 0)),
            Self::NotEqual => Some((1, 0x10, 0)),
            Self::LessEqual => Some((1, 0x28, 4)),
            Self::GreaterEqual => Some((1, 0x28, 2)),
            Self::Symbol(b'<') => Some((1, 0x28, 3)),
            Self::Symbol(b'>') => Some((1, 0x28, 0)),
            Self::Symbol(op @ (b'+' | b'-')) => Some((2, 0x11, op)),
            Self::Symbol(op @ (b'*' | b'/' | b'%')) => Some((3, 0x11, op)),
            Self::Return
            | Self::Local
            | Self::If
            | Self::Else
            | Self::While
            | Self::Break
            | Self::Continue
            | Self::Identifier(_)
            | Self::Scalar(_)
            | Self::Symbol(_)
            | Self::And
            | Self::Or
            | Self::End => None,
        }
    }
}
