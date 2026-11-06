
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
    let tokens = tokenize(
        "message = \"Hello\"\nactive = true"
    );

    assert!(tokens.contains(&TokenKind::String));
    assert!(tokens.contains(&TokenKind::Boolean));
}

#[test]
fn recognizes_indentation() {
    let tokens = tokenize(
        "if true:\n    score = 10\nstop"
    );

    assert!(tokens.contains(&TokenKind::Indent));
    assert!(tokens.contains(&TokenKind::Dedent));
}

#[test]
fn ignores_comments() {
    let tokens = tokenize(
        "score = 10 # this is a comment"
    );

    assert!(!tokens.contains(&TokenKind::Identifier)
        || tokens.iter().filter(|t| **t == TokenKind::Identifier).count() == 1);
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
        tokens.iter().filter(|token| token.kind == TokenKind::Newline).count(),
        2
    );
}
