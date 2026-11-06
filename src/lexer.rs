
use crate::tokens::{Token, TokenKind};

pub struct Lexer {
    source: Vec<char>,
    current: usize,
    line: usize,
    column: usize,
    indent_stack: Vec<usize>,
    tokens: Vec<Token>,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.chars().collect(),
            current: 0,
            line: 1,
            column: 1,
            indent_stack: vec![0],
            tokens: Vec::new(),
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        while !self.is_at_end() {
            if self.column == 1 {
                self.handle_indentation()?;

                if self.is_at_end() {
                    break;
                }

                if self.peek() == Some('\n') {
                    self.advance();
                    continue;
                }

                if self.peek() == Some('#') {
                    self.skip_comment();

                    if self.peek() == Some('\n') {
                        self.advance();
                    }

                    continue;
                }
            }

            match self.peek() {
                Some(' ') => {
                    self.advance();
                }

                Some('\t') => {
                    return Err(format!(
                        "Unexpected tab at line {}, column {}.",
                        self.line, self.column
                    ));
                }

                Some('\n') => {
                    let line = self.line;
                    let column = self.column;

                    self.advance();

                    self.tokens.push(Token::new(
                        TokenKind::Newline,
                        "\n".to_string(),
                        line,
                        column,
                    ));
                }

                // Accept Windows line endings as one logical newline.
                Some('\r') if self.peek_next() == Some('\n') => {
                    let line = self.line;
                    let column = self.column;

                    self.advance();
                    self.advance();

                    self.tokens.push(Token::new(
                        TokenKind::Newline,
                        "\r\n".to_string(),
                        line,
                        column,
                    ));
                }

                Some('#') => {
                    self.skip_comment();
                }

                Some('"') | Some('\'') => {
                    self.read_string()?;
                }

                Some(c) if c.is_ascii_digit() => {
                    self.read_number();
                }

                Some(c) if c.is_alphabetic() || c == '_' => {
                    self.read_identifier();
                }

                Some('+') => {
                    self.read_operator(
                        TokenKind::Plus,
                        Some(('=', TokenKind::PlusEqual)),
                    );
                }

                Some('-') => {
                    self.read_operator(
                        TokenKind::Minus,
                        Some(('=', TokenKind::MinusEqual)),
                    );
                }

                Some('*') => {
                    self.read_operator(
                        TokenKind::Multiply,
                        Some(('=', TokenKind::MultiplyEqual)),
                    );
                }

                Some('/') => {
                    self.read_operator(
                        TokenKind::Divide,
                        Some(('=', TokenKind::DivideEqual)),
                    );
                }

                Some('=') => {
                    self.read_operator(
                        TokenKind::Equal,
                        Some(('=', TokenKind::EqualEqual)),
                    );
                }

                Some('!') => {
                    self.advance();

                    if self.peek() == Some('=') {
                        self.advance();
                        self.add_token_from(
                            TokenKind::NotEqual,
                            self.current - 2,
                            self.line,
                            self.column - 2,
                        );
                    } else {
                        return Err(format!(
                            "Unexpected '!' at line {}, column {}.",
                            self.line,
                            self.column - 1
                        ));
                    }
                }

                Some('<') => {
                    self.read_operator(
                        TokenKind::LessThan,
                        Some(('=', TokenKind::LessEqual)),
                    );
                }

                Some('>') => {
                    self.read_operator(
                        TokenKind::GreaterThan,
                        Some(('=', TokenKind::GreaterEqual)),
                    );
                }

                Some('(') => self.single_char(TokenKind::LeftParen),
                Some(')') => self.single_char(TokenKind::RightParen),
                Some(':') => self.single_char(TokenKind::Colon),
                Some(',') => self.single_char(TokenKind::Comma),
                Some(';') => self.single_char(TokenKind::Semicolon),
                Some('.') => self.single_char(TokenKind::Dot),
                Some('[') => self.single_char(TokenKind::LeftBracket),
                Some(']') => self.single_char(TokenKind::RightBracket),
                Some('{') => self.single_char(TokenKind::LeftBrace),
                Some('}') => self.single_char(TokenKind::RightBrace),

                Some(c) => {
                    return Err(format!(
                        "Unexpected character '{}' at line {}, column {}.",
                        c, self.line, self.column
                    ));
                }

                None => break,
            }
        }

        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();

            self.add_token_at(
                TokenKind::Dedent,
                self.line,
                self.column,
            );
        }

        self.add_token_at(
            TokenKind::Eof,
            self.line,
            self.column,
        );

        Ok(self.tokens.clone())
    }

    fn handle_indentation(&mut self) -> Result<(), String> {
        let start_column = self.column;
        let mut indentation = 0;

        while self.peek() == Some(' ') {
            indentation += 1;
            self.advance();
        }

        if self.peek() == Some('\t') {
            return Err(format!(
                "Tabs are not allowed for indentation at line {}.",
                self.line
            ));
        }

        if matches!(self.peek(), Some('\n') | None | Some('#')) {
            return Ok(());
        }

        let current_indent = *self.indent_stack.last().unwrap();

        if indentation > current_indent {
            let difference = indentation - current_indent;

            if difference > 4 {
                return Err(format!(
                    "Indentation increased by more than 4 spaces at line {}.",
                    self.line
                ));
            }

            self.indent_stack.push(indentation);

            self.add_token_at(
                TokenKind::Indent,
                self.line,
                start_column,
            );
        } else if indentation < current_indent {
            while indentation < *self.indent_stack.last().unwrap() {
                self.indent_stack.pop();

                self.add_token_at(
                    TokenKind::Dedent,
                    self.line,
                    start_column,
                );
            }

            if indentation != *self.indent_stack.last().unwrap() {
                return Err(format!(
                    "Inconsistent indentation at line {}.",
                    self.line
                ));
            }
        }

        Ok(())
    }

    fn read_number(&mut self) {
        let start = self.current;
        let start_line = self.line;
        let start_column = self.column;

        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
        }

        let mut kind = TokenKind::Integer;

        if self.peek() == Some('.')
            && self.peek_next().is_some_and(|c| c.is_ascii_digit())
        {
            kind = TokenKind::Float;
            self.advance();

            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
        }

        self.add_token_from(
            kind,
            start,
            start_line,
            start_column,
        );
    }

    fn read_string(&mut self) -> Result<(), String> {
        let quote = self.peek().unwrap();
        let start = self.current;
        let start_line = self.line;
        let start_column = self.column;

        self.advance();

        while let Some(c) = self.peek() {
            if c == quote {
                self.advance();

                self.add_token_from(
                    TokenKind::String,
                    start,
                    start_line,
                    start_column,
                );

                return Ok(());
            }

            if c == '\n' {
                return Err(format!(
                    "Unterminated string at line {}, column {}.",
                    start_line, start_column
                ));
            }

            if c == '\\' {
                self.advance();

                if self.peek().is_none() {
                    break;
                }
            }

            self.advance();
        }

        Err(format!(
            "Unterminated string at line {}, column {}.",
            start_line, start_column
        ))
    }

    fn read_identifier(&mut self) {
        let start = self.current;
        let start_line = self.line;
        let start_column = self.column;

        while self.peek().is_some_and(|c| {
            c.is_alphanumeric() || c == '_'
        }) {
            self.advance();
        }

        let text: String = self.source[start..self.current]
            .iter()
            .collect();

        let kind = match text.as_str() {
            "true" | "false" => TokenKind::Boolean,

            "if" => TokenKind::If,
            "elif" => TokenKind::Elif,
            "else" => TokenKind::Else,

            "loop" => TokenKind::Loop,
            "repeat" => TokenKind::Repeat,
            "delay" => TokenKind::Delay,

            "function" => TokenKind::Function,
            "return" => TokenKind::Return,
            "break" => TokenKind::Break,
            "stop" => TokenKind::Stop,

            "get" => TokenKind::Get,
            "from" => TokenKind::From,

            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,

            _ => TokenKind::Identifier,
        };

        self.add_token_from(
            kind,
            start,
            start_line,
            start_column,
        );
    }

    fn read_operator(
        &mut self,
        normal_kind: TokenKind,
        compound: Option<(char, TokenKind)>,
    ) {
        let start = self.current;
        let start_line = self.line;
        let start_column = self.column;

        self.advance();

        let kind = if let Some((expected, compound_kind)) = compound {
            if self.peek() == Some(expected) {
                self.advance();
                compound_kind
            } else {
                normal_kind
            }
        } else {
            normal_kind
        };

        self.add_token_from(
            kind,
            start,
            start_line,
            start_column,
        );
    }

    fn single_char(&mut self, kind: TokenKind) {
        let start = self.current;
        let line = self.line;
        let column = self.column;

        self.advance();

        self.add_token_from(
            kind,
            start,
            line,
            column,
        );
    }

    fn skip_comment(&mut self) {
        while !self.is_at_end() && self.peek() != Some('\n') {
            self.advance();
        }
    }

    fn add_token_from(
        &mut self,
        kind: TokenKind,
        start: usize,
        line: usize,
        column: usize,
    ) {
        let lexeme: String = self.source[start..self.current]
            .iter()
            .collect();

        self.tokens.push(Token::new(
            kind,
            lexeme,
            line,
            column,
        ));
    }

    fn add_token_at(
        &mut self,
        kind: TokenKind,
        line: usize,
        column: usize,
    ) {
        self.tokens.push(Token::new(
            kind,
            String::new(),
            line,
            column,
        ));
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;

        self.current += 1;

        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }

        Some(c)
    }

    fn peek(&self) -> Option<char> {
        self.source.get(self.current).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.source.get(self.current + 1).copied()
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }
}
