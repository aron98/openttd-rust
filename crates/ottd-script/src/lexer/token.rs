//! Compiler tokens; unsupported reserved words are rejected during lexing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind<'a> {
    Const,
    Root,
    NewSlot,
    Enum,
    Return,
    TypeOf,
    Local,
    If,
    Else,
    While,
    Switch,
    Case,
    Default,
    For,
    Do,
    Compound(u8),
    Increment(i8),
    Shift(u8),
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
#[derive(Clone, Debug)]
pub(crate) struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub offset: usize,
    pub newline: bool,
}
impl TokenKind<'_> {
    pub(crate) const fn binary(&self) -> Option<(u8, u8, u8)> {
        match self {
            Self::Symbol(b'|') => Some((1, 0x12, 2)),
            Self::Symbol(b'^') => Some((2, 0x12, 3)),
            Self::Symbol(b'&') => Some((3, 0x12, 0)),
            Self::Shift(selector) => Some((5, 0x12, *selector)),
            Self::Equal => Some((4, 0x0f, 0)),
            Self::NotEqual => Some((4, 0x10, 0)),
            Self::LessEqual => Some((4, 0x28, 4)),
            Self::GreaterEqual => Some((4, 0x28, 2)),
            Self::Symbol(b'<') => Some((4, 0x28, 3)),
            Self::Symbol(b'>') => Some((4, 0x28, 0)),
            Self::Symbol(op @ (b'+' | b'-')) => Some((6, 0x11, *op)),
            Self::Symbol(op @ (b'*' | b'/' | b'%')) => Some((7, 0x11, *op)),
            Self::Const
            | Self::Root
            | Self::NewSlot
            | Self::Enum
            | Self::TypeOf
            | Self::Return
            | Self::Local
            | Self::If
            | Self::Else
            | Self::While
            | Self::Switch
            | Self::Case
            | Self::Default
            | Self::For
            | Self::Do
            | Self::Compound(_)
            | Self::Increment(_)
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
