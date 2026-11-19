use nova::ast::{AssignmentOperator, BinaryOperator, Expression, Statement};
use nova::lexer::Lexer;
use nova::parser::Parser;
use nova::runtime::{Runtime, apply_binary};
use nova::value::Value;

fn parse(source: &str) -> nova::ast::Program {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("source should tokenize");
    let mut parser = Parser::new(tokens);
    parser.parse().expect("source should parse")
}

fn execute_assignments(source: &str) -> Runtime {
    let program = parse(source);
    let mut runtime = Runtime::new();
    runtime
        .execute_program(&program)
        .expect("core program should execute");
    runtime
}

#[test]
fn variables_can_be_reassigned_to_a_different_type() {
    let runtime = execute_assignments("age = 15\nage = \"fifteen\"\nstop");

    assert_eq!(runtime.get("age").unwrap(), &Value::from("fifteen"));
}

#[test]
fn expression_evaluation_respects_parser_precedence() {
    let runtime = execute_assignments("result = 10 + 5 * 2\nstop");

    assert_eq!(runtime.get("result").unwrap(), &Value::Int(20));
}

#[test]
fn numeric_arithmetic_promotes_mixed_values_and_division_to_float() {
    assert_eq!(
        apply_binary(BinaryOperator::Add, Value::Int(1), Value::Float(2.5)).unwrap(),
        Value::Float(3.5),
    );
    assert_eq!(
        apply_binary(BinaryOperator::Divide, Value::Int(5), Value::Int(2)).unwrap(),
        Value::Float(2.5),
    );
}

#[test]
fn strings_can_be_concatenated() {
    assert_eq!(
        apply_binary(
            BinaryOperator::Add,
            Value::from("Nova "),
            Value::from("works"),
        )
        .unwrap(),
        Value::from("Nova works"),
    );
}

#[test]
fn print_emits_one_space_joined_output_line() {
    let runtime = execute_assignments("print(\"Nova\", 7, true)\nstop");
    assert_eq!(runtime.output(), &["Nova 7 true".to_string()]);
}

#[test]
fn comparisons_and_boolean_operators_return_booleans() {
    let runtime = execute_assignments("result = 3 < 5 and not false\nstop");
    assert_eq!(runtime.get("result").unwrap(), &Value::Bool(true));
}

#[test]
fn mixed_numeric_comparisons_keep_integer_precision() {
    let result = apply_binary(
        BinaryOperator::GreaterThan,
        Value::Int(9_007_199_254_740_993),
        Value::Float(9_007_199_254_740_992.0),
    )
    .unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn compound_assignment_updates_a_dynamic_variable() {
    let runtime = execute_assignments("score = 10\nscore += 5\nstop");
    assert_eq!(runtime.get("score").unwrap(), &Value::Int(15));
}

#[test]
fn core_runtime_executes_collection_indexing_and_mutation() {
    let runtime = execute_assignments(
        "items = [[1, 2]]\nitems[0][1] += 4\nsettings = {\"enabled\": false}\nsettings[\"enabled\"] = items[0][1] == 6\nstop",
    );
    assert_eq!(
        runtime.get("items").unwrap(),
        &Value::List(vec![Value::List(vec![Value::Int(1), Value::Int(6)])])
    );
    assert_eq!(
        runtime.get("settings").unwrap(),
        &Value::Dictionary(vec![(Value::from("enabled"), Value::Bool(true))])
    );
}

#[test]
fn core_runtime_executes_source_conversion_functions() {
    let runtime = execute_assignments(
        "whole = int(\"12\")\nratio = float(3)\nrendered = str(true)\nflag = bool([])\nstop",
    );
    assert_eq!(runtime.get("whole").unwrap(), &Value::Int(12));
    assert_eq!(runtime.get("ratio").unwrap(), &Value::Float(3.0));
    assert_eq!(runtime.get("rendered").unwrap(), &Value::from("true"));
    assert_eq!(runtime.get("flag").unwrap(), &Value::Bool(false));
}

#[test]
fn unsupported_operations_return_errors() {
    let error =
        apply_binary(BinaryOperator::Subtract, Value::from("ten"), Value::Int(2)).unwrap_err();
    assert!(error.to_string().contains("not supported"));
}

#[test]
fn division_by_zero_returns_an_error() {
    let error = apply_binary(BinaryOperator::Divide, Value::Int(10), Value::Int(0)).unwrap_err();
    assert_eq!(error.to_string(), "Division by zero.");
}

#[test]
fn ast_runtime_rejects_host_injected_non_finite_values() {
    let mut runtime = Runtime::new();
    runtime.assign("external", Value::Float(f64::INFINITY));
    let program = parse("print(external)\nstop");
    let error = runtime.execute_program(&program).unwrap_err();
    assert!(error.to_string().contains("Non-finite Float values"));
}

#[test]
fn missing_variables_return_errors() {
    let mut runtime = Runtime::new();
    let error = runtime
        .evaluate_expression(&Expression::Identifier("missing".to_string()))
        .unwrap_err();
    assert!(error.to_string().contains("Undefined variable"));
}

#[test]
fn unsupported_control_flow_returns_a_runtime_error() {
    let program = parse("if true:\n    print(\"yes\")\nstop");
    let mut runtime = Runtime::new();
    assert!(runtime.execute_program(&program).is_err());
}

#[test]
fn compound_assignment_requires_an_existing_variable() {
    let program = parse("score += 1\nstop");
    let Statement::Assignment {
        name,
        operator,
        value,
    } = program.statements[0].unlocated()
    else {
        panic!("Expected an assignment");
    };
    assert!(matches!(operator, AssignmentOperator::Add));

    let mut runtime = Runtime::new();
    assert!(runtime.execute_assignment(name, *operator, value).is_err());
}

#[test]
fn integer_overflow_returns_an_error() {
    let error = apply_binary(BinaryOperator::Add, Value::Int(i64::MAX), Value::Int(1)).unwrap_err();
    assert!(error.to_string().contains("Integer overflow"));
}
