use crate::ast::{
    AssignmentOperator, BinaryOperator, Expression, LoopSettings, Program, Statement, UnaryOperator,
};
use crate::tokens::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    pub fn parse(&mut self) -> Result<Program, String> {
        if self.tokens.is_empty() {
            return Err("Cannot parse an empty token stream.".to_string());
        }
        if self.tokens.last().map(|token| &token.kind) != Some(&TokenKind::Eof)
            || self.tokens[..self.tokens.len() - 1]
                .iter()
                .any(|token| token.kind == TokenKind::Eof)
        {
            return Err("Token stream must end with exactly one EOF token.".into());
        }

        let mut statements = Vec::new();
        let mut found_stop = false;

        self.skip_newlines();

        while !self.is_at_end() {
            if found_stop {
                return self.error("No statements are allowed after 'stop'.");
            }

            let statement = self.parse_statement()?;

            if matches!(statement.unlocated(), Statement::Stop) {
                found_stop = true;
            }

            statements.push(statement);
            self.skip_newlines();
        }

        if !found_stop {
            return self.error("Every Nova program must end with 'stop'.");
        }

        Ok(Program { statements })
    }

    fn parse_statement(&mut self) -> Result<Statement, String> {
        let location = self
            .tokens
            .get(self.current)
            .map(|token| crate::tokens::SourceLocation {
                line: token.line,
                column: token.column,
            });
        let statement = match self.peek_kind() {
            Some(TokenKind::If) => self.parse_if(),
            Some(TokenKind::Loop) => self.parse_loop(),
            Some(TokenKind::Function) => self.parse_function(),
            Some(TokenKind::Return) => self.parse_return(),
            Some(TokenKind::Break) => {
                self.advance();
                Ok(Statement::Break)
            }
            Some(TokenKind::Stop) => {
                self.advance();
                Ok(Statement::Stop)
            }
            Some(TokenKind::Get) => self.parse_import(),
            Some(TokenKind::From) => self.parse_specific_import(),
            _ => self.parse_assignment_or_expression(),
        }?;
        Ok(match location {
            Some(location) => Statement::Located {
                location,
                statement: Box::new(statement),
            },
            None => statement,
        })
    }

    // ---------------------------------------------------------
    // Variables and assignments
    // ---------------------------------------------------------

    fn parse_assignment_or_expression(&mut self) -> Result<Statement, String> {
        if self.looks_like_unpack_assignment() {
            let mut names = vec![self.consume_identifier("Expected variable name.")?];
            while self.match_token(TokenKind::Comma) {
                names.push(self.consume_identifier("Expected variable name after ','.")?);
            }
            self.consume(TokenKind::Equal, "Expected '=' after variable names.")?;
            let value = self.parse_expression(0)?;
            return Ok(Statement::UnpackAssignment { names, value });
        }

        if self.looks_like_index_assignment() {
            let name = self.advance().lexeme.clone();
            let mut indices = Vec::new();
            while self.match_token(TokenKind::LeftBracket) {
                indices.push(self.parse_expression(0)?);
                self.consume(TokenKind::RightBracket, "Expected ']' after index.")?;
            }
            let operator = match self.advance().kind {
                TokenKind::Equal => AssignmentOperator::Assign,
                TokenKind::PlusEqual => AssignmentOperator::Add,
                TokenKind::MinusEqual => AssignmentOperator::Subtract,
                TokenKind::MultiplyEqual => AssignmentOperator::Multiply,
                TokenKind::DivideEqual => AssignmentOperator::Divide,
                _ => unreachable!(),
            };
            let value = self.parse_expression(0)?;
            return Ok(Statement::IndexAssignment {
                name,
                indices,
                operator,
                value,
            });
        }

        if self.check(TokenKind::Identifier)
            && matches!(
                self.peek_next_kind(),
                Some(
                    TokenKind::Equal
                        | TokenKind::PlusEqual
                        | TokenKind::MinusEqual
                        | TokenKind::MultiplyEqual
                        | TokenKind::DivideEqual
                )
            )
        {
            let name = self.advance().lexeme.clone();
            let operator = match self.advance().kind {
                TokenKind::Equal => AssignmentOperator::Assign,
                TokenKind::PlusEqual => AssignmentOperator::Add,
                TokenKind::MinusEqual => AssignmentOperator::Subtract,
                TokenKind::MultiplyEqual => AssignmentOperator::Multiply,
                TokenKind::DivideEqual => AssignmentOperator::Divide,
                _ => unreachable!(),
            };

            let value = self.parse_expression(0)?;

            return Ok(Statement::Assignment {
                name,
                operator,
                value,
            });
        }

        let expression = self.parse_expression(0)?;

        Ok(Statement::Expression(expression))
    }

    // ---------------------------------------------------------
    // Conditionals
    // ---------------------------------------------------------

    fn parse_if(&mut self) -> Result<Statement, String> {
        self.consume(TokenKind::If, "Expected 'if'.")?;

        let condition = self.parse_expression(0)?;
        self.consume(TokenKind::Colon, "Expected ':' after condition.")?;

        let body = self.parse_block()?;
        let mut elif_branches = Vec::new();
        let mut else_body = None;

        loop {
            self.skip_newlines();

            if self.match_token(TokenKind::Elif) {
                let condition = self.parse_expression(0)?;

                self.consume(TokenKind::Colon, "Expected ':' after elif condition.")?;

                let body = self.parse_block()?;
                elif_branches.push((condition, body));
            } else if self.match_token(TokenKind::Else) {
                self.consume(TokenKind::Colon, "Expected ':' after else.")?;

                else_body = Some(self.parse_block()?);
                break;
            } else {
                break;
            }
        }

        Ok(Statement::If {
            condition,
            body,
            elif_branches,
            else_body,
        })
    }

    // ---------------------------------------------------------
    // Loops
    // ---------------------------------------------------------

    fn parse_loop(&mut self) -> Result<Statement, String> {
        self.consume(TokenKind::Loop, "Expected 'loop'.")?;

        let settings = if self.match_token(TokenKind::Colon) {
            LoopSettings {
                condition: None,
                max_iterations: None,
                delay: None,
            }
        } else {
            self.consume(TokenKind::Dot, "Expected ':' or '.repeat' after loop.")?;

            self.consume(TokenKind::Repeat, "Expected 'repeat' after 'loop.'.")?;

            let condition = self.parse_expression(0)?;
            let mut max_iterations = None;
            let mut delay = None;

            if self.match_token(TokenKind::Semicolon) {
                max_iterations = Some(self.parse_expression(0)?);

                if self.match_token(TokenKind::Comma) {
                    delay = Some(self.parse_delay()?);
                }
            } else if self.match_token(TokenKind::Comma) {
                if self.check(TokenKind::Delay) {
                    delay = Some(self.parse_delay()?);
                } else {
                    max_iterations = Some(self.parse_expression(0)?);

                    if self.match_token(TokenKind::Comma) {
                        delay = Some(self.parse_delay()?);
                    }
                }
            }

            self.consume(TokenKind::Colon, "Expected ':' after loop settings.")?;

            LoopSettings {
                condition: Some(condition),
                max_iterations,
                delay,
            }
        };

        let body = self.parse_block()?;

        Ok(Statement::Loop { settings, body })
    }

    fn parse_delay(&mut self) -> Result<Expression, String> {
        self.consume(TokenKind::Delay, "Expected 'delay'.")?;

        self.consume(TokenKind::LeftParen, "Expected '(' after delay.")?;

        let duration = self.parse_expression(0)?;

        self.consume(TokenKind::RightParen, "Expected ')' after delay duration.")?;

        Ok(duration)
    }

    // ---------------------------------------------------------
    // Functions and returns
    // ---------------------------------------------------------

    fn parse_function(&mut self) -> Result<Statement, String> {
        self.consume(TokenKind::Function, "Expected 'function'.")?;

        let name = self.consume_identifier("Expected function name.")?;

        self.consume(TokenKind::LeftParen, "Expected '(' after function name.")?;

        let mut parameters = Vec::new();

        if !self.check(TokenKind::RightParen) {
            loop {
                parameters.push(self.consume_identifier("Expected parameter name.")?);

                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }

        self.consume(TokenKind::RightParen, "Expected ')' after parameters.")?;

        self.consume(TokenKind::Colon, "Expected ':' after function declaration.")?;

        let body = self.parse_block()?;

        Ok(Statement::Function {
            name,
            parameters,
            body,
        })
    }

    fn parse_return(&mut self) -> Result<Statement, String> {
        self.consume(TokenKind::Return, "Expected 'return'.")?;

        if self.check(TokenKind::Newline)
            || self.check(TokenKind::Dedent)
            || self.check(TokenKind::Eof)
        {
            return Ok(Statement::Return(None));
        }

        let parenthesized = self.match_token(TokenKind::LeftParen);
        if parenthesized && self.match_token(TokenKind::RightParen) {
            return Ok(Statement::Return(None));
        }

        let mut values = vec![self.parse_expression(0)?];

        while self.match_token(TokenKind::Comma) {
            values.push(self.parse_expression(0)?);
        }

        if parenthesized {
            self.consume(TokenKind::RightParen, "Expected ')' after return values.")?;
        }

        Ok(Statement::Return(Some(values)))
    }

    // ---------------------------------------------------------
    // Imports
    // ---------------------------------------------------------

    fn parse_import(&mut self) -> Result<Statement, String> {
        self.consume(TokenKind::Get, "Expected 'get'.")?;

        let module = self.consume_identifier("Expected module name after 'get'.")?;

        Ok(Statement::Import { module, item: None })
    }

    fn parse_specific_import(&mut self) -> Result<Statement, String> {
        self.consume(TokenKind::From, "Expected 'from'.")?;

        let module = self.consume_identifier("Expected module name after 'from'.")?;

        self.consume(TokenKind::Get, "Expected 'get' after module name.")?;

        let item = self.consume_identifier("Expected item name after 'get'.")?;

        Ok(Statement::Import {
            module,
            item: Some(item),
        })
    }

    // ---------------------------------------------------------
    // Indentation-based blocks
    // ---------------------------------------------------------

    fn parse_block(&mut self) -> Result<Vec<Statement>, String> {
        self.skip_newlines();

        self.consume(TokenKind::Indent, "Expected an indented block.")?;

        let mut statements = Vec::new();

        self.skip_newlines();

        while !self.check(TokenKind::Dedent) && !self.is_at_end() {
            statements.push(self.parse_statement()?);
            self.skip_newlines();
        }

        self.consume(TokenKind::Dedent, "Expected the end of the indented block.")?;

        Ok(statements)
    }

    // ---------------------------------------------------------
    // Expressions and operator precedence
    // ---------------------------------------------------------

    fn parse_expression(&mut self, min_precedence: u8) -> Result<Expression, String> {
        let start_location =
            self.tokens
                .get(self.current)
                .map(|token| crate::tokens::SourceLocation {
                    line: token.line,
                    column: token.column,
                });
        let mut left = self.parse_unary()?;

        while let Some((operator, precedence)) = self.current_binary_operator() {
            if precedence < min_precedence {
                break;
            }

            let operator_location =
                self.tokens
                    .get(self.current)
                    .map(|token| crate::tokens::SourceLocation {
                        line: token.line,
                        column: token.column,
                    });
            self.advance();

            let right = self.parse_expression(precedence + 1)?;

            let binary = Expression::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
            left = match operator_location {
                Some(location) => Expression::Located {
                    location,
                    expression: Box::new(binary),
                },
                None => binary,
            };
        }

        Ok(match start_location {
            Some(location) => Expression::Located {
                location,
                expression: Box::new(left),
            },
            None => left,
        })
    }

    fn parse_unary(&mut self) -> Result<Expression, String> {
        if self.match_token(TokenKind::Minus) {
            let operand = self.parse_unary()?;

            return Ok(Expression::Unary {
                operator: UnaryOperator::Negative,
                operand: Box::new(operand),
            });
        }

        if self.match_token(TokenKind::Not) {
            let operand = self.parse_unary()?;

            return Ok(Expression::Unary {
                operator: UnaryOperator::Not,
                operand: Box::new(operand),
            });
        }

        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expression, String> {
        let mut expression = self.parse_primary()?;
        loop {
            if self.match_token(TokenKind::LeftBracket) {
                let index = self.parse_expression(0)?;
                self.consume(TokenKind::RightBracket, "Expected ']' after index.")?;
                expression = Expression::Index {
                    object: Box::new(expression),
                    index: Box::new(index),
                };
            } else if self.match_token(TokenKind::Dot) {
                let name = self.consume_identifier("Expected a member name after '.'.")?;
                expression = Expression::Member {
                    object: Box::new(expression),
                    name,
                };
                if self.match_token(TokenKind::LeftParen) {
                    let call_name = qualified_name(&expression).ok_or_else(|| {
                        "Only named functions can be called through a module.".to_string()
                    })?;
                    expression = self.finish_call(call_name)?;
                }
            } else {
                break;
            }
        }
        Ok(expression)
    }

    fn current_binary_operator(&self) -> Option<(BinaryOperator, u8)> {
        match self.peek_kind()? {
            TokenKind::Or => Some((BinaryOperator::Or, 1)),
            TokenKind::And => Some((BinaryOperator::And, 2)),

            TokenKind::EqualEqual => Some((BinaryOperator::Equal, 3)),
            TokenKind::NotEqual => Some((BinaryOperator::NotEqual, 3)),
            TokenKind::LessThan => Some((BinaryOperator::LessThan, 3)),
            TokenKind::GreaterThan => Some((BinaryOperator::GreaterThan, 3)),
            TokenKind::LessEqual => Some((BinaryOperator::LessEqual, 3)),
            TokenKind::GreaterEqual => Some((BinaryOperator::GreaterEqual, 3)),

            TokenKind::Plus => Some((BinaryOperator::Add, 4)),
            TokenKind::Minus => Some((BinaryOperator::Subtract, 4)),

            TokenKind::Multiply => Some((BinaryOperator::Multiply, 5)),
            TokenKind::Divide => Some((BinaryOperator::Divide, 5)),

            _ => None,
        }
    }

    fn parse_primary(&mut self) -> Result<Expression, String> {
        let token = self.advance().clone();

        match token.kind {
            TokenKind::Integer => {
                let value = token
                    .lexeme
                    .parse::<i64>()
                    .map_err(|_| format!("Invalid integer at line {}.", token.line))?;

                Ok(Expression::Integer(value))
            }

            TokenKind::Float => {
                let value = token.lexeme.parse::<f64>().map_err(|_| {
                    format!(
                        "Invalid float at line {}, column {}.",
                        token.line, token.column
                    )
                })?;
                if !value.is_finite() {
                    return Err(format!(
                        "Float literal must be finite at line {}, column {}.",
                        token.line, token.column
                    ));
                }

                Ok(Expression::Float(value))
            }

            TokenKind::String => {
                let value = decode_string_literal(&token.lexeme).map_err(|error| {
                    format!("{} At line {}, column {}.", error, token.line, token.column)
                })?;

                Ok(Expression::String(value))
            }

            TokenKind::Boolean => Ok(Expression::Boolean(token.lexeme == "true")),

            TokenKind::Identifier => {
                let name = token.lexeme;

                if self.match_token(TokenKind::LeftParen) {
                    self.finish_call(name)
                } else {
                    Ok(Expression::Identifier(name))
                }
            }

            TokenKind::Delay => {
                self.consume(TokenKind::LeftParen, "Expected '(' after delay.")?;

                let argument = self.parse_expression(0)?;

                self.consume(TokenKind::RightParen, "Expected ')' after delay argument.")?;

                Ok(Expression::Call {
                    name: "delay".to_string(),
                    arguments: vec![argument],
                })
            }

            TokenKind::LeftParen => {
                let expression = self.parse_expression(0)?;

                self.consume(TokenKind::RightParen, "Expected ')' after expression.")?;

                Ok(Expression::Grouped(Box::new(expression)))
            }

            TokenKind::LeftBracket => self.parse_list(),

            TokenKind::LeftBrace => self.parse_dictionary(),

            _ => Err(format!(
                "Expected an expression at line {}, column {}.",
                token.line, token.column
            )),
        }
    }

    // ---------------------------------------------------------
    // Function calls
    // ---------------------------------------------------------

    fn finish_call(&mut self, name: String) -> Result<Expression, String> {
        let mut arguments = Vec::new();

        if !self.check(TokenKind::RightParen) {
            loop {
                arguments.push(self.parse_expression(0)?);

                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }

        self.consume(
            TokenKind::RightParen,
            "Expected ')' after function arguments.",
        )?;

        Ok(Expression::Call { name, arguments })
    }

    // ---------------------------------------------------------
    // Lists
    // ---------------------------------------------------------

    fn parse_list(&mut self) -> Result<Expression, String> {
        let mut elements = Vec::new();

        if !self.check(TokenKind::RightBracket) {
            loop {
                elements.push(self.parse_expression(0)?);

                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }

        self.consume(TokenKind::RightBracket, "Expected ']' after list elements.")?;

        Ok(Expression::List(elements))
    }

    // ---------------------------------------------------------
    // Dictionaries
    // ---------------------------------------------------------

    fn parse_dictionary(&mut self) -> Result<Expression, String> {
        let mut entries = Vec::new();

        if !self.check(TokenKind::RightBrace) {
            loop {
                let key = self.parse_expression(0)?;

                self.consume(
                    TokenKind::Colon,
                    "Expected ':' between dictionary key and value.",
                )?;

                let value = self.parse_expression(0)?;

                entries.push((key, value));

                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }

        self.consume(
            TokenKind::RightBrace,
            "Expected '}' after dictionary entries.",
        )?;

        Ok(Expression::Dictionary(entries))
    }

    // ---------------------------------------------------------
    // Token utilities
    // ---------------------------------------------------------

    fn consume_identifier(&mut self, message: &str) -> Result<String, String> {
        if self.check(TokenKind::Identifier) {
            Ok(self.advance().lexeme.clone())
        } else {
            self.error(message)
        }
    }

    fn looks_like_unpack_assignment(&self) -> bool {
        if !self.check(TokenKind::Identifier) {
            return false;
        }

        let mut index = self.current + 1;
        if self.tokens.get(index).map(|token| &token.kind) != Some(&TokenKind::Comma) {
            return false;
        }

        loop {
            index += 1;
            if self.tokens.get(index).map(|token| &token.kind) != Some(&TokenKind::Identifier) {
                return false;
            }
            index += 1;
            match self.tokens.get(index).map(|token| &token.kind) {
                Some(TokenKind::Comma) => {}
                Some(TokenKind::Equal) => return true,
                _ => return false,
            }
        }
    }

    fn looks_like_index_assignment(&self) -> bool {
        if !self.check(TokenKind::Identifier) {
            return false;
        }

        let mut cursor = self.current + 1;
        loop {
            if self.tokens.get(cursor).map(|token| &token.kind) != Some(&TokenKind::LeftBracket) {
                return false;
            }

            let mut closers = vec![TokenKind::RightBracket];
            cursor += 1;
            while let Some(token) = self.tokens.get(cursor) {
                match token.kind {
                    TokenKind::LeftParen => closers.push(TokenKind::RightParen),
                    TokenKind::LeftBracket => closers.push(TokenKind::RightBracket),
                    TokenKind::LeftBrace => closers.push(TokenKind::RightBrace),
                    TokenKind::RightParen | TokenKind::RightBracket | TokenKind::RightBrace => {
                        if closers.last() != Some(&token.kind) {
                            return false;
                        }
                        closers.pop();
                        if closers.is_empty() {
                            cursor += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                cursor += 1;
            }

            match self.tokens.get(cursor).map(|token| &token.kind) {
                Some(
                    TokenKind::Equal
                    | TokenKind::PlusEqual
                    | TokenKind::MinusEqual
                    | TokenKind::MultiplyEqual
                    | TokenKind::DivideEqual,
                ) => return true,
                Some(TokenKind::LeftBracket) => {}
                _ => return false,
            }
        }
    }

    fn consume(&mut self, kind: TokenKind, message: &str) -> Result<&Token, String> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            self.error(message)
        }
    }

    fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, kind: TokenKind) -> bool {
        self.peek_kind() == Some(kind)
    }

    fn peek_kind(&self) -> Option<TokenKind> {
        self.tokens.get(self.current).map(|t| t.kind.clone())
    }

    fn peek_next_kind(&self) -> Option<TokenKind> {
        self.tokens.get(self.current + 1).map(|t| t.kind.clone())
    }

    fn advance(&mut self) -> &Token {
        if self.current < self.tokens.len() && !self.is_at_end() {
            self.current += 1;
        }

        // Parser::new is public, so callers may provide an empty or
        // otherwise incomplete token stream. Keep that a parse error rather
        // than indexing before the start of the vector.
        let index = self
            .current
            .saturating_sub(1)
            .min(self.tokens.len().saturating_sub(1));
        &self.tokens[index]
    }

    fn is_at_end(&self) -> bool {
        self.peek_kind() == Some(TokenKind::Eof)
    }

    fn skip_newlines(&mut self) {
        while self.match_token(TokenKind::Newline) {}
    }

    fn error<T>(&self, message: &str) -> Result<T, String> {
        if let Some(token) = self.tokens.get(self.current) {
            Err(format!(
                "{} At line {}, column {}.",
                message, token.line, token.column
            ))
        } else {
            Err(message.to_string())
        }
    }
}

fn qualified_name(expression: &Expression) -> Option<String> {
    match expression {
        Expression::Identifier(name) => Some(name.clone()),
        Expression::Member { object, name } => {
            Some(format!("{}.{}", qualified_name(object)?, name))
        }
        _ => None,
    }
}

fn decode_string_literal(literal: &str) -> Result<String, String> {
    let mut characters = literal.chars();
    let quote = characters
        .next()
        .ok_or_else(|| "Invalid empty string token.".to_string())?;
    if !matches!(quote, '\'' | '"') || characters.next_back() != Some(quote) {
        return Err("Invalid string token delimiters.".into());
    }

    let mut decoded = String::new();
    let mut content = characters.peekable();
    while let Some(character) = content.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        let escaped = content
            .next()
            .ok_or_else(|| "Unterminated escape sequence.".to_string())?;
        decoded.push(match escaped {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            '\\' => '\\',
            '"' => '"',
            '\'' => '\'',
            other => return Err(format!("Unsupported escape sequence '\\{}'.", other)),
        });
    }
    Ok(decoded)
}
