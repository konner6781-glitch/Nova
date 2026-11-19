use nova::lexer::Lexer;
use nova::tokens::TokenKind;

fn tokenize(source: &str) -> Vec<TokenKind> {
    let mut lexer = Lexer::new(source);

    lexer
        .tokenize()
        .expect("Lexer failed")
        .iter()
        .map(|token| token.kind.clone())
        .collect()
}

#[test]
fn recognizes_variables_and_numbers() {
    let tokens = tokenize("score = 25 + 10");

    assert_eq!(
        tokens,
        vec![
            TokenKind::Identifier,
            TokenKind::Equal,
            TokenKind::Integer,
            TokenKind::Plus,
            TokenKind::Integer,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn recognizes_strings_and_booleans() {
    let tokens = tokenize("message = \"Hello\"\nactive = true");

    assert!(tokens.contains(&TokenKind::String));
    assert!(tokens.contains(&TokenKind::Boolean));
}

#[test]
fn recognizes_indentation() {
    let tokens = tokenize("if true:\n    score = 10\nstop");

    assert!(tokens.contains(&TokenKind::Indent));
    assert!(tokens.contains(&TokenKind::Dedent));
}

#[test]
fn ignores_comments() {
    let tokens = Lexer::new("score = 10 # this is a comment")
        .tokenize()
        .expect("comment should tokenize");
    assert_eq!(
        tokens
            .iter()
            .map(|token| (token.kind.clone(), token.lexeme.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (TokenKind::Identifier, "score"),
            (TokenKind::Equal, "="),
            (TokenKind::Integer, "10"),
            (TokenKind::Eof, ""),
        ]
    );
}

#[test]
fn recognizes_supported_string_escape_sequences() {
    let tokens = Lexer::new(r#"value = "\n\t\r\\\"\'""#)
        .tokenize()
        .expect("supported escapes should tokenize");
    let string = tokens
        .iter()
        .find(|token| token.kind == TokenKind::String)
        .unwrap();
    assert_eq!(string.lexeme, r#""\n\t\r\\\"\'""#);
}

#[test]
fn rejects_unsupported_escape_sequences_with_a_location() {
    let error = Lexer::new("value = \"bad\\q\"").tokenize().unwrap_err();
    assert!(error.contains("Unsupported escape sequence"));
    assert!(error.contains("line 1, column 13"));
}

#[test]
fn rejects_invalid_characters() {
    let mut lexer = Lexer::new("@");

    assert!(lexer.tokenize().is_err());
}

#[test]
fn rejects_unterminated_strings() {
    let mut lexer = Lexer::new("\"Hello");

    assert!(lexer.tokenize().is_err());
}

#[test]
fn accepts_windows_crlf_line_endings() {
    let mut lexer = Lexer::new("score = 10\r\nstop\r\n");
    let tokens = lexer.tokenize().expect("CRLF source should tokenize");
    assert_eq!(
        tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Newline)
            .count(),
        2
    );
}

#[test]
fn ignores_whitespace_only_crlf_lines_without_changing_indentation() {
    let source = "if true:\r\n    print(1)\r\n    \r\nstop\r\n";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("blank CRLF line should not change indentation");
    assert_eq!(
        tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Indent)
            .count(),
        1
    );
    assert_eq!(
        tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Dedent)
            .count(),
        1
    );
}
