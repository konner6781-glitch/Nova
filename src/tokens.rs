
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    // Values
    Integer,
    Float,
    String,
    Boolean,
    Identifier,

    // Keywords
    If,
    Elif,
    Else,
    Loop,
    Repeat,
    Delay,
    Function,
    Return,
    Break,
    Stop,
    Get,
    From,

    // Logical operators
    And,
    Or,
    Not,

    // Arithmetic operators
    Plus,
    Minus,
    Multiply,
    Divide,

    // Compound assignment
    PlusEqual,
    MinusEqual,
    MultiplyEqual,
    DivideEqual,

    // Comparison and assignment
    Equal,
    EqualEqual,
    NotEqual,
    LessThan,
    GreaterThan,
    LessEqual,
    GreaterEqual,

    // Punctuation
    LeftParen,
    RightParen,
    Colon,
    Comma,
    Semicolon,
    Dot,

    // Collections
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,

    // Code structure
    Newline,
    Indent,
    Dedent,

    // End of source code
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
}

impl Token {
    pub fn new(
        kind: TokenKind,
        lexeme: String,
        line: usize,
        column: usize,
    ) -> Self {
        Self {
            kind,
            lexeme,
            line,
            column,
        }
    }
}
