
use nova::ast::{
    AssignmentOperator, BinaryOperator, Expression,
    Statement, UnaryOperator,
};
use nova::lexer::Lexer;
use nova::parser::Parser;

fn parse(source: &str) -> Result<nova::ast::Program, String> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;

    let mut parser = Parser::new(tokens);
    parser.parse()
}

#[test]
fn parses_variable_assignments() {
    let program = parse(
        "score = 100\nname = \"Nova\"\nstop"
    ).unwrap();

    assert_eq!(program.statements.len(), 3);

    assert!(matches!(
        &program.statements[0],
        Statement::Assignment {
            name,
            operator: AssignmentOperator::Assign,
            value: Expression::Integer(100),
        } if name == "score"
    ));

    assert!(matches!(
        &program.statements[1],
        Statement::Assignment {
            name,
            value: Expression::String(value),
            ..
        } if name == "name" && value == "Nova"
    ));
}

#[test]
fn parses_compound_assignments() {
    let program = parse(
        "score = 10\nscore += 5\nscore -= 2\nstop"
    ).unwrap();

    assert!(matches!(
        &program.statements[1],
        Statement::Assignment {
            operator: AssignmentOperator::Add,
            ..
        }
    ));

    assert!(matches!(
        &program.statements[2],
        Statement::Assignment {
            operator: AssignmentOperator::Subtract,
            ..
        }
    ));
}

#[test]
fn parses_arithmetic_precedence() {
    let program = parse(
        "result = 2 + 3 * 4\nstop"
    ).unwrap();

    let Statement::Assignment { value, .. } =
        &program.statements[0]
    else {
        panic!("Expected assignment");
    };

    let Expression::Binary {
        operator: BinaryOperator::Add,
        right,
        ..
    } = value
    else {
        panic!("Expected addition at the top level");
    };

    assert!(matches!(
        right.as_ref(),
        Expression::Binary {
            operator: BinaryOperator::Multiply,
            ..
        }
    ));
}

#[test]
fn parses_comparisons_and_logical_operators() {
    let program = parse(
        "result = score > 10 and score < 100\nstop"
    ).unwrap();

    let Statement::Assignment { value, .. } =
        &program.statements[0]
    else {
        panic!("Expected assignment");
    };

    assert!(matches!(
        value,
        Expression::Binary {
            operator: BinaryOperator::And,
            ..
        }
    ));
}

#[test]
fn parses_unary_operators() {
    let program = parse(
        "negative = -10\nactive = not false\nstop"
    ).unwrap();

    let Statement::Assignment {
        value: negative,
        ..
    } = &program.statements[0]
    else {
        panic!("Expected assignment");
    };

    assert!(matches!(
        negative,
        Expression::Unary {
            operator: UnaryOperator::Negative,
            ..
        }
    ));

    let Statement::Assignment {
        value: active,
        ..
    } = &program.statements[1]
    else {
        panic!("Expected assignment");
    };

    assert!(matches!(
        active,
        Expression::Unary {
            operator: UnaryOperator::Not,
            ..
        }
    ));
}

#[test]
fn parses_if_else() {
    let program = parse(
        "if score > 50:\n    print(\"Passed\")\nelse:\n    print(\"Failed\")\nstop"
    ).unwrap();

    assert!(matches!(
        &program.statements[0],
        Statement::If {
            else_body: Some(_),
            ..
        }
    ));
}

#[test]
fn parses_elif_branches() {
    let program = parse(
        "if score > 90:\n    print(\"A\")\nelif score > 80:\n    print(\"B\")\nelif score > 70:\n    print(\"C\")\nelse:\n    print(\"Try again\")\nstop"
    ).unwrap();

    let Statement::If {
        elif_branches,
        else_body,
        ..
    } = &program.statements[0]
    else {
        panic!("Expected if statement");
    };

    assert_eq!(elif_branches.len(), 2);
    assert!(else_body.is_some());
}

#[test]
fn parses_nested_conditionals() {
    let program = parse(
        "if true:\n    if false:\n        print(\"A\")\n    else:\n        print(\"B\")\nstop"
    ).unwrap();

    let Statement::If { body, .. } =
        &program.statements[0]
    else {
        panic!("Expected outer if statement");
    };

    assert!(matches!(
        &body[0],
        Statement::If { .. }
    ));
}

#[test]
fn parses_all_loop_formats() {
    let sources = [
        "loop:\n    print(\"Infinite\")\nstop",
        "loop.repeat x < 10:\n    print(x)\nstop",
        "loop.repeat x < 10, 5:\n    print(x)\nstop",
        "loop.repeat x < 10, delay(1):\n    print(x)\nstop",
        "loop.repeat x < 10; 5, delay(1):\n    print(x)\nstop",
    ];

    for (index, source) in sources.iter().enumerate() {
        let program = parse(source)
            .unwrap_or_else(|error| {
                panic!("Loop format {} failed: {}", index + 1, error)
            });

        assert!(matches!(
            &program.statements[0],
            Statement::Loop { .. }
        ));
    }
}

#[test]
fn parses_loop_settings() {
    let program = parse(
        "loop.repeat x < 10; 5, delay(1):\n    print(x)\nstop"
    ).unwrap();

    let Statement::Loop { settings, .. } =
        &program.statements[0]
    else {
        panic!("Expected loop");
    };

    assert!(settings.condition.is_some());
    assert!(settings.max_iterations.is_some());
    assert!(settings.delay.is_some());
}

#[test]
fn parses_function_declarations() {
    let program = parse(
        "function add(a, b):\n    return a + b\nstop"
    ).unwrap();

    let Statement::Function {
        name,
        parameters,
        body,
    } = &program.statements[0]
    else {
        panic!("Expected function declaration");
    };

    assert_eq!(name, "add");
    assert_eq!(parameters.len(), 2);
    assert_eq!(body.len(), 1);
    assert!(matches!(
        &body[0],
        Statement::Return(Some(_))
    ));
}

#[test]
fn parses_multiple_return_values() {
    let program = parse(
        "function coordinates():\n    return 10, 20\nstop"
    ).unwrap();

    let Statement::Function { body, .. } =
        &program.statements[0]
    else {
        panic!("Expected function");
    };

    assert!(matches!(
        &body[0],
        Statement::Return(Some(values))
            if values.len() == 2
    ));
}

#[test]
fn parses_function_calls_with_multiple_arguments() {
    let program = parse(
        "print(\"Hello\", 10, true)\nstop"
    ).unwrap();

    assert!(matches!(
        &program.statements[0],
        Statement::Expression(
            Expression::Call {
                name,
                arguments
            }
        ) if name == "print" && arguments.len() == 3
    ));
}

#[test]
fn parses_lists() {
    let program = parse(
        "numbers = [10, 20, 30]\nstop"
    ).unwrap();

    assert!(matches!(
        &program.statements[0],
        Statement::Assignment {
            value: Expression::List(elements),
            ..
        } if elements.len() == 3
    ));
}

#[test]
fn parses_dictionaries() {
    let program = parse(
        "player = {\"name\": \"Nova\", \"score\": 100}\nstop"
    ).unwrap();

    assert!(matches!(
        &program.statements[0],
        Statement::Assignment {
            value: Expression::Dictionary(entries),
            ..
        } if entries.len() == 2
    ));
}

#[test]
fn parses_imports() {
    let program = parse(
        "get tokens\nfrom Math get sqrt\nstop"
    ).unwrap();

    assert!(matches!(
        &program.statements[0],
        Statement::Import {
            module,
            item: None
        } if module == "tokens"
    ));

    assert!(matches!(
        &program.statements[1],
        Statement::Import {
            module,
            item: Some(item)
        } if module == "Math" && item == "sqrt"
    ));
}

#[test]
fn rejects_missing_stop() {
    assert!(parse("score = 100").is_err());
}

#[test]
fn rejects_statements_after_stop() {
    assert!(parse(
        "score = 100\nstop\nprint(\"Too late\")"
    ).is_err());
}

#[test]
fn rejects_invalid_syntax() {
    assert!(parse(
        "if true\n    print(\"Missing colon\")\nstop"
    ).is_err());
}

#[test]
fn empty_token_stream_returns_error_without_panicking() {
    let mut parser = Parser::new(Vec::new());
    assert!(parser.parse().is_err());
}
