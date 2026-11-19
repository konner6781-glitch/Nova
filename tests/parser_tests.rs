use nova::ast::{AssignmentOperator, BinaryOperator, Expression, Statement, UnaryOperator};
use nova::lexer::Lexer;
use nova::parser::Parser;

fn parse(source: &str) -> Result<nova::ast::Program, String> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;

    let mut parser = Parser::new(tokens);
    parser.parse().map(strip_program_locations)
}

fn strip_program_locations(mut program: nova::ast::Program) -> nova::ast::Program {
    program.statements = program
        .statements
        .into_iter()
        .map(strip_statement)
        .collect();
    program
}

fn strip_statement(statement: Statement) -> Statement {
    match statement {
        Statement::Located { statement, .. } => strip_statement(*statement),
        Statement::Variable { name, value } => Statement::Variable {
            name,
            value: strip_expression(value),
        },
        Statement::Assignment {
            name,
            operator,
            value,
        } => Statement::Assignment {
            name,
            operator,
            value: strip_expression(value),
        },
        Statement::IndexAssignment {
            name,
            indices,
            operator,
            value,
        } => Statement::IndexAssignment {
            name,
            indices: indices.into_iter().map(strip_expression).collect(),
            operator,
            value: strip_expression(value),
        },
        Statement::UnpackAssignment { names, value } => Statement::UnpackAssignment {
            names,
            value: strip_expression(value),
        },
        Statement::Expression(expression) => Statement::Expression(strip_expression(expression)),
        Statement::If {
            condition,
            body,
            elif_branches,
            else_body,
        } => Statement::If {
            condition: strip_expression(condition),
            body: body.into_iter().map(strip_statement).collect(),
            elif_branches: elif_branches
                .into_iter()
                .map(|(condition, body)| {
                    (
                        strip_expression(condition),
                        body.into_iter().map(strip_statement).collect(),
                    )
                })
                .collect(),
            else_body: else_body.map(|body| body.into_iter().map(strip_statement).collect()),
        },
        Statement::Loop { settings, body } => Statement::Loop {
            settings: nova::ast::LoopSettings {
                condition: settings.condition.map(strip_expression),
                max_iterations: settings.max_iterations.map(strip_expression),
                delay: settings.delay.map(strip_expression),
            },
            body: body.into_iter().map(strip_statement).collect(),
        },
        Statement::Function {
            name,
            parameters,
            body,
        } => Statement::Function {
            name,
            parameters,
            body: body.into_iter().map(strip_statement).collect(),
        },
        Statement::Return(values) => Statement::Return(
            values.map(|values| values.into_iter().map(strip_expression).collect()),
        ),
        Statement::Break => Statement::Break,
        Statement::Import { module, item } => Statement::Import { module, item },
        Statement::Stop => Statement::Stop,
    }
}

fn strip_expression(expression: Expression) -> Expression {
    match expression {
        Expression::Located { expression, .. } => strip_expression(*expression),
        Expression::Integer(value) => Expression::Integer(value),
        Expression::Float(value) => Expression::Float(value),
        Expression::String(value) => Expression::String(value),
        Expression::Boolean(value) => Expression::Boolean(value),
        Expression::Identifier(name) => Expression::Identifier(name),
        Expression::List(values) => {
            Expression::List(values.into_iter().map(strip_expression).collect())
        }
        Expression::Dictionary(entries) => Expression::Dictionary(
            entries
                .into_iter()
                .map(|(key, value)| (strip_expression(key), strip_expression(value)))
                .collect(),
        ),
        Expression::Index { object, index } => Expression::Index {
            object: Box::new(strip_expression(*object)),
            index: Box::new(strip_expression(*index)),
        },
        Expression::Member { object, name } => Expression::Member {
            object: Box::new(strip_expression(*object)),
            name,
        },
        Expression::Unary { operator, operand } => Expression::Unary {
            operator,
            operand: Box::new(strip_expression(*operand)),
        },
        Expression::Binary {
            left,
            operator,
            right,
        } => Expression::Binary {
            left: Box::new(strip_expression(*left)),
            operator,
            right: Box::new(strip_expression(*right)),
        },
        Expression::Call { name, arguments } => Expression::Call {
            name,
            arguments: arguments.into_iter().map(strip_expression).collect(),
        },
        Expression::Grouped(expression) => {
            Expression::Grouped(Box::new(strip_expression(*expression)))
        }
    }
}

#[test]
fn parses_variable_assignments() {
    let program = parse("score = 100\nname = \"Nova\"\nstop").unwrap();

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
    let program = parse("score = 10\nscore += 5\nscore -= 2\nstop").unwrap();

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
fn parses_collection_reads_and_assignments() {
    let program = parse("items = [[1, 2]]\nitems[0][1] += 3\nresult = items[0][1]\nstop").unwrap();

    assert!(matches!(
        &program.statements[1],
        Statement::IndexAssignment {
            name,
            indices,
            operator: AssignmentOperator::Add,
            ..
        } if name == "items" && indices.len() == 2
    ));
    assert!(matches!(
        &program.statements[2],
        Statement::Assignment {
            value: Expression::Index { .. },
            ..
        }
    ));
}

#[test]
fn parses_arithmetic_precedence() {
    let program = parse("result = 2 + 3 * 4\nstop").unwrap();

    let Statement::Assignment { value, .. } = &program.statements[0] else {
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
    let program = parse("result = score > 10 and score < 100\nstop").unwrap();

    let Statement::Assignment { value, .. } = &program.statements[0] else {
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
    let program = parse("negative = -10\nactive = not false\nstop").unwrap();

    let Statement::Assignment {
        value: negative, ..
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

    let Statement::Assignment { value: active, .. } = &program.statements[1] else {
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
    let program =
        parse("if score > 50:\n    print(\"Passed\")\nelse:\n    print(\"Failed\")\nstop").unwrap();

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
        "if true:\n    if false:\n        print(\"A\")\n    else:\n        print(\"B\")\nstop",
    )
    .unwrap();

    let Statement::If { body, .. } = &program.statements[0] else {
        panic!("Expected outer if statement");
    };

    assert!(matches!(&body[0], Statement::If { .. }));
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
            .unwrap_or_else(|error| panic!("Loop format {} failed: {}", index + 1, error));

        assert!(matches!(&program.statements[0], Statement::Loop { .. }));
    }
}

#[test]
fn parses_loop_settings() {
    let program = parse("loop.repeat x < 10; 5, delay(1):\n    print(x)\nstop").unwrap();

    let Statement::Loop { settings, .. } = &program.statements[0] else {
        panic!("Expected loop");
    };

    assert!(settings.condition.is_some());
    assert!(settings.max_iterations.is_some());
    assert!(settings.delay.is_some());
}

#[test]
fn parses_function_declarations() {
    let program = parse("function add(a, b):\n    return a + b\nstop").unwrap();

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
    assert!(matches!(&body[0], Statement::Return(Some(_))));
}

#[test]
fn parses_multiple_return_values() {
    let program = parse("function coordinates():\n    return 10, 20\nstop").unwrap();

    let Statement::Function { body, .. } = &program.statements[0] else {
        panic!("Expected function");
    };

    assert!(matches!(
        &body[0],
        Statement::Return(Some(values))
            if values.len() == 2
    ));
}

#[test]
fn parses_parenthesized_multiple_return_values() {
    let program = parse("function coordinates():\n    return(10, 20)\nstop").unwrap();

    let Statement::Function { body, .. } = &program.statements[0] else {
        panic!("Expected function");
    };
    assert!(matches!(
        &body[0],
        Statement::Return(Some(values)) if values.len() == 2
    ));
}

#[test]
fn parses_unpack_assignments() {
    let program = parse("first, second = coordinates()\nstop").unwrap();
    assert!(matches!(
        &program.statements[0],
        Statement::UnpackAssignment { names, value: Expression::Call { .. } }
            if names == &["first".to_string(), "second".to_string()]
    ));
}

#[test]
fn parses_function_calls_with_multiple_arguments() {
    let program = parse("print(\"Hello\", 10, true)\nstop").unwrap();

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
    let program = parse("numbers = [10, 20, 30]\nstop").unwrap();

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
    let program = parse("player = {\"name\": \"Nova\", \"score\": 100}\nstop").unwrap();

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
    let program = parse("get tokens\nfrom Math get sqrt\nstop").unwrap();

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
fn parses_module_member_reads_and_calls() {
    let program = parse("get Math\nroot = Math.sqrt(16)\nconstant = Math.pi\nstop").unwrap();
    assert!(matches!(
        &program.statements[1],
        Statement::Assignment {
            value: Expression::Call { name, arguments },
            ..
        } if name == "Math.sqrt" && arguments.len() == 1
    ));
    assert!(matches!(
        &program.statements[2],
        Statement::Assignment {
            value: Expression::Member { name, .. },
            ..
        } if name == "pi"
    ));
}

#[test]
fn rejects_missing_stop() {
    assert!(parse("score = 100").is_err());
}

#[test]
fn rejects_statements_after_stop() {
    assert!(parse("score = 100\nstop\nprint(\"Too late\")").is_err());
}

#[test]
fn rejects_invalid_syntax() {
    assert!(parse("if true\n    print(\"Missing colon\")\nstop").is_err());
}

#[test]
fn empty_token_stream_returns_error_without_panicking() {
    let mut parser = Parser::new(Vec::new());
    assert!(parser.parse().is_err());
}

#[test]
fn decodes_string_escapes_exactly_once() {
    let source = r#"value = "n:\n t:\t r:\r slash:\\ quote:\" apostrophe:\'"
stop"#;
    let program = parse(source).expect("escaped string should parse");
    let Statement::Assignment {
        value: Expression::String(value),
        ..
    } = &program.statements[0]
    else {
        panic!("expected string assignment");
    };
    assert_eq!(value, "n:\n t:\t r:\r slash:\\ quote:\" apostrophe:'");

    let source = r#"value = 'single: \' double: \"'
stop"#;
    let program = parse(source).expect("single-quoted escapes should parse");
    let Statement::Assignment {
        value: Expression::String(value),
        ..
    } = &program.statements[0]
    else {
        panic!("expected string assignment");
    };
    assert_eq!(value, "single: ' double: \"");
}

#[test]
fn rejects_float_literals_that_overflow_to_infinity() {
    let huge = "9".repeat(400);
    let source = format!("value = {}.0\nstop", huge);
    let error = parse(&source).unwrap_err();
    assert!(error.to_ascii_lowercase().contains("float"));
    assert!(error.contains("line 1, column 9"));
}

#[test]
fn rejects_token_stream_without_a_single_trailing_eof() {
    let tokens = Lexer::new("stop")
        .tokenize()
        .expect("source should tokenize");
    let without_eof = tokens[..tokens.len() - 1].to_vec();
    assert!(
        Parser::new(without_eof)
            .parse()
            .unwrap_err()
            .contains("EOF")
    );
}

#[test]
fn parser_preserves_statement_expression_and_operator_locations() {
    let tokens = Lexer::new("result = 10 / 0\nstop")
        .tokenize()
        .expect("source should tokenize");
    let program = Parser::new(tokens).parse().expect("source should parse");
    assert_eq!(
        program.statements[0].location(),
        Some(nova::tokens::SourceLocation { line: 1, column: 1 })
    );
    let Statement::Assignment { value, .. } = program.statements[0].unlocated() else {
        panic!("expected assignment");
    };
    assert_eq!(
        value.location(),
        Some(nova::tokens::SourceLocation {
            line: 1,
            column: 10
        })
    );
    let Expression::Located { expression, .. } = value else {
        panic!("expected operator location");
    };
    assert_eq!(
        expression.location(),
        Some(nova::tokens::SourceLocation {
            line: 1,
            column: 13
        })
    );
}
