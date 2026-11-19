use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;

use crate::ast::{
    AssignmentOperator, BinaryOperator, Expression, Program, Statement, UnaryOperator,
};
use crate::tokens::SourceLocation;
use crate::value::{Value, normalize_dictionary};

/// An error raised while evaluating Nova runtime values and expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError {
    message: String,
    location: Option<SourceLocation>,
}

impl RuntimeError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            location: None,
        }
    }

    fn with_location(mut self, location: Option<SourceLocation>) -> Self {
        if self.location.is_none() {
            self.location = location;
        }
        self
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn location(&self) -> Option<SourceLocation> {
        self.location
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)?;
        if let Some(location) = self.location {
            write!(
                formatter,
                " at line {}, column {}",
                location.line, location.column
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for RuntimeError {}

/// The core runtime state for Nova's dynamically typed variables.
///
/// Assignment creates a variable when it does not exist and replaces its value
/// when it does. Compound assignments require an existing variable.
#[derive(Debug, Clone, Default)]
pub struct Runtime {
    variables: HashMap<String, Value>,
    output: Vec<String>,
}

impl Runtime {
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates or replaces a variable without restricting its value type.
    pub fn assign(&mut self, name: impl Into<String>, value: Value) {
        self.variables.insert(name.into(), value);
    }

    /// Reads a variable or returns a runtime error for an unknown name.
    pub fn get(&self, name: &str) -> Result<&Value, RuntimeError> {
        self.variables
            .get(name)
            .ok_or_else(|| RuntimeError::new(format!("Undefined variable '{}'.", name)))
    }

    /// Returns each line emitted by the Nova print function.
    pub fn output(&self) -> &[String] {
        &self.output
    }

    /// Takes all output lines emitted so far.
    pub fn take_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.output)
    }

    /// Executes the currently supported top-level statements.
    pub fn execute_program(&mut self, program: &Program) -> Result<(), RuntimeError> {
        for statement in &program.statements {
            let result = match statement.unlocated() {
                Statement::Variable { name, value } => self
                    .execute_assignment(name, AssignmentOperator::Assign, value)
                    .map(|_| ()),
                Statement::Assignment {
                    name,
                    operator,
                    value,
                } => self.execute_assignment(name, *operator, value).map(|_| ()),
                Statement::IndexAssignment {
                    name,
                    indices,
                    operator,
                    value,
                } => self.execute_index_assignment(name, indices, *operator, value),
                Statement::Expression(expression) => {
                    self.evaluate_expression(expression).map(|_| ())
                }
                Statement::Stop => break,
                _ => Err(RuntimeError::new(
                    "Execution of this statement is not implemented in the core runtime.",
                )),
            };
            result.map_err(|error| error.with_location(statement.location()))?;
        }

        Ok(())
    }

    /// Executes one parsed assignment, including compound assignment.
    pub fn execute_assignment(
        &mut self,
        name: &str,
        operator: AssignmentOperator,
        expression: &Expression,
    ) -> Result<Value, RuntimeError> {
        let previous = match operator {
            AssignmentOperator::Assign => None,
            _ => Some(self.get(name)?.clone()),
        };
        let right = self.evaluate_expression(expression)?;
        let value = match (operator, previous) {
            (AssignmentOperator::Assign, None) => right,
            (
                AssignmentOperator::Add
                | AssignmentOperator::Subtract
                | AssignmentOperator::Multiply
                | AssignmentOperator::Divide,
                Some(left),
            ) => {
                let binary_operator = match operator {
                    AssignmentOperator::Add => BinaryOperator::Add,
                    AssignmentOperator::Subtract => BinaryOperator::Subtract,
                    AssignmentOperator::Multiply => BinaryOperator::Multiply,
                    AssignmentOperator::Divide => BinaryOperator::Divide,
                    AssignmentOperator::Assign => unreachable!(),
                };
                apply_binary(binary_operator, left, right)?
            }
            _ => unreachable!("assignment operator and previous value must agree"),
        };

        self.assign(name, value.clone());
        Ok(value)
    }

    fn execute_index_assignment(
        &mut self,
        name: &str,
        index_expressions: &[Expression],
        operator: AssignmentOperator,
        expression: &Expression,
    ) -> Result<(), RuntimeError> {
        let mut indices = Vec::with_capacity(index_expressions.len());
        for index in index_expressions {
            indices.push(self.evaluate_expression(index)?);
        }
        let right = self.evaluate_expression(expression)?;
        let collection = self.get(name)?.clone();
        let replacement = if operator == AssignmentOperator::Assign {
            right
        } else {
            let previous = read_index_path(&collection, &indices)?;
            let binary_operator = match operator {
                AssignmentOperator::Add => BinaryOperator::Add,
                AssignmentOperator::Subtract => BinaryOperator::Subtract,
                AssignmentOperator::Multiply => BinaryOperator::Multiply,
                AssignmentOperator::Divide => BinaryOperator::Divide,
                AssignmentOperator::Assign => unreachable!(),
            };
            apply_binary(binary_operator, previous, right)?
        };
        self.assign(name, write_index_path(collection, &indices, replacement)?);
        Ok(())
    }

    /// Evaluates literals, variable reads, unary operations, and binary
    /// operations from the existing AST.
    pub fn evaluate_expression(&mut self, expression: &Expression) -> Result<Value, RuntimeError> {
        match expression {
            Expression::Located {
                location,
                expression,
            } => self
                .evaluate_expression(expression)
                .map_err(|error| error.with_location(Some(*location))),
            Expression::Integer(value) => Ok(Value::Int(*value)),
            Expression::Float(value) if value.is_finite() => Ok(Value::Float(*value)),
            Expression::Float(_) => Err(RuntimeError::new("Float literal must be finite.")),
            Expression::String(value) => Ok(Value::String(value.clone())),
            Expression::Boolean(value) => Ok(Value::Bool(*value)),
            Expression::Identifier(name) => {
                let value = self.get(name)?.clone();
                value.validate_finite().map_err(RuntimeError::new)?;
                Ok(value)
            }
            Expression::Grouped(expression) => self.evaluate_expression(expression),
            Expression::Unary { operator, operand } => {
                let value = self.evaluate_expression(operand)?;
                apply_unary(*operator, value)
            }
            Expression::Binary {
                left,
                operator: BinaryOperator::And,
                right,
            } => {
                let left = self.evaluate_expression(left)?;
                if !left.to_bool().map_err(RuntimeError::new)? {
                    return Ok(Value::Bool(false));
                }
                let right = self.evaluate_expression(right)?;
                Ok(Value::Bool(right.to_bool().map_err(RuntimeError::new)?))
            }
            Expression::Binary {
                left,
                operator: BinaryOperator::Or,
                right,
            } => {
                let left = self.evaluate_expression(left)?;
                if left.to_bool().map_err(RuntimeError::new)? {
                    return Ok(Value::Bool(true));
                }
                let right = self.evaluate_expression(right)?;
                Ok(Value::Bool(right.to_bool().map_err(RuntimeError::new)?))
            }
            Expression::Binary {
                left,
                operator,
                right,
            } => {
                let left = self.evaluate_expression(left)?;
                let right = self.evaluate_expression(right)?;
                apply_binary(*operator, left, right)
            }
            Expression::Call { name, arguments } if name == "print" => {
                let mut rendered = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    rendered.push(self.evaluate_expression(argument)?.to_string());
                }
                self.output.push(rendered.join(" "));
                Ok(Value::None)
            }
            Expression::Call { name, arguments }
                if matches!(name.as_str(), "int" | "float" | "str" | "bool") =>
            {
                if arguments.len() != 1 {
                    return Err(RuntimeError::new(format!(
                        "Function '{}' expects 1 argument but received {}.",
                        name,
                        arguments.len()
                    )));
                }
                let value = self.evaluate_expression(&arguments[0])?;
                match name.as_str() {
                    "int" => value.to_int().map(Value::Int).map_err(RuntimeError::new),
                    "float" => value
                        .to_float()
                        .map(Value::Float)
                        .map_err(RuntimeError::new),
                    "str" => Ok(Value::String(value.to_string_value())),
                    "bool" => value.to_bool().map(Value::Bool).map_err(RuntimeError::new),
                    _ => unreachable!(),
                }
            }
            Expression::Call { name, .. } => Err(RuntimeError::new(format!(
                "Function call '{}' is not implemented in the core runtime.",
                name
            ))),
            Expression::List(elements) => elements
                .iter()
                .map(|element| self.evaluate_expression(element))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::List),
            Expression::Dictionary(entries) => {
                let mut values: Vec<(Value, Value)> = Vec::with_capacity(entries.len());
                for (key, value) in entries {
                    let key = self.evaluate_expression(key)?;
                    let value = self.evaluate_expression(value)?;
                    if let Some((_, old_value)) =
                        values.iter_mut().find(|(old_key, _)| old_key == &key)
                    {
                        *old_value = value;
                    } else {
                        values.push((key, value));
                    }
                }
                Ok(Value::Dictionary(values))
            }
            Expression::Index { object, index } => {
                let object = self.evaluate_expression(object)?;
                let index = self.evaluate_expression(index)?;
                read_index(&object, &index)
            }
            Expression::Member { object, name } => {
                let object = self.evaluate_expression(object)?;
                read_index(&object, &Value::String(name.clone()))
            }
        }
    }
}

fn read_index_path(collection: &Value, indices: &[Value]) -> Result<Value, RuntimeError> {
    let mut value = collection.clone();
    for index in indices {
        value = read_index(&value, index)?;
    }
    Ok(value)
}

fn write_index_path(
    collection: Value,
    indices: &[Value],
    replacement: Value,
) -> Result<Value, RuntimeError> {
    let Some((index, remainder)) = indices.split_first() else {
        return Err(RuntimeError::new(
            "Index assignment requires at least one index.",
        ));
    };
    if remainder.is_empty() {
        return write_index(collection, index, replacement);
    }
    let child = read_index(&collection, index)?;
    let updated_child = write_index_path(child, remainder, replacement)?;
    write_index(collection, index, updated_child)
}

fn read_index(collection: &Value, index: &Value) -> Result<Value, RuntimeError> {
    match collection {
        Value::List(values) => {
            let position = list_position(index, values.len())?;
            Ok(values[position].clone())
        }
        Value::Dictionary(entries) => entries
            .iter()
            .rev()
            .find(|(key, _)| key == index)
            .map(|(_, value)| value.clone())
            .ok_or_else(|| RuntimeError::new(format!("Dictionary has no key '{}'.", index))),
        value => Err(RuntimeError::new(format!(
            "Cannot index a {} value.",
            value.type_name()
        ))),
    }
}

fn write_index(
    collection: Value,
    index: &Value,
    replacement: Value,
) -> Result<Value, RuntimeError> {
    match collection {
        Value::List(mut values) => {
            let position = list_position(index, values.len())?;
            values[position] = replacement;
            Ok(Value::List(values))
        }
        Value::Dictionary(entries) => {
            let mut entries = normalize_dictionary(&entries);
            if let Some((_, value)) = entries.iter_mut().find(|(key, _)| key == index) {
                *value = replacement;
            } else {
                entries.push((index.clone(), replacement));
            }
            Ok(Value::Dictionary(entries))
        }
        value => Err(RuntimeError::new(format!(
            "Cannot assign an index of a {} value.",
            value.type_name()
        ))),
    }
}

fn list_position(index: &Value, length: usize) -> Result<usize, RuntimeError> {
    let Value::Int(index) = index else {
        return Err(RuntimeError::new(format!(
            "List indices must be Int values, received {}.",
            index.type_name()
        )));
    };
    if *index < 0 || *index as u128 >= length as u128 {
        return Err(RuntimeError::new(format!(
            "List index {} is out of range for length {}.",
            index, length
        )));
    }
    Ok(*index as usize)
}

/// Applies a binary AST operator to two already evaluated values.
pub fn apply_binary(
    operator: BinaryOperator,
    left: Value,
    right: Value,
) -> Result<Value, RuntimeError> {
    left.validate_finite().map_err(RuntimeError::new)?;
    right.validate_finite().map_err(RuntimeError::new)?;
    match operator {
        BinaryOperator::Add => add(left, right),
        BinaryOperator::Subtract => arithmetic(left, right, Arithmetic::Subtract),
        BinaryOperator::Multiply => arithmetic(left, right, Arithmetic::Multiply),
        BinaryOperator::Divide => divide(left, right),
        BinaryOperator::Equal => Ok(Value::Bool(left == right)),
        BinaryOperator::NotEqual => Ok(Value::Bool(left != right)),
        BinaryOperator::LessThan
        | BinaryOperator::GreaterThan
        | BinaryOperator::LessEqual
        | BinaryOperator::GreaterEqual => compare(operator, left, right),
        BinaryOperator::And | BinaryOperator::Or => {
            let left = left.to_bool().map_err(RuntimeError::new)?;
            let right = right.to_bool().map_err(RuntimeError::new)?;
            Ok(Value::Bool(match operator {
                BinaryOperator::And => left && right,
                BinaryOperator::Or => left || right,
                _ => unreachable!(),
            }))
        }
    }
}

/// Applies a unary AST operator to an already evaluated value.
pub fn apply_unary(operator: UnaryOperator, value: Value) -> Result<Value, RuntimeError> {
    value.validate_finite().map_err(RuntimeError::new)?;
    match operator {
        UnaryOperator::Not => Ok(Value::Bool(!value.to_bool().map_err(RuntimeError::new)?)),
        UnaryOperator::Negative => match value {
            Value::Int(value) => value
                .checked_neg()
                .map(Value::Int)
                .ok_or_else(|| RuntimeError::new("Integer overflow during negation.")),
            Value::Float(value) => checked_float(-value, "negation"),
            value => Err(type_error("unary -", &value, None)),
        },
    }
}

#[derive(Clone, Copy)]
enum Arithmetic {
    Subtract,
    Multiply,
}

#[derive(Clone, Copy)]
enum Number {
    Int(i64),
    Float(f64),
}

fn number(value: &Value) -> Option<Number> {
    match value {
        Value::Int(value) => Some(Number::Int(*value)),
        Value::Float(value) => Some(Number::Float(*value)),
        _ => None,
    }
}

fn add(left: Value, right: Value) -> Result<Value, RuntimeError> {
    if let (Value::String(left), Value::String(right)) = (&left, &right) {
        return Ok(Value::String(format!("{left}{right}")));
    }

    arithmetic_values(left, right, "+", |a, b| a.checked_add(b), |a, b| a + b)
}

fn arithmetic(left: Value, right: Value, operation: Arithmetic) -> Result<Value, RuntimeError> {
    match operation {
        Arithmetic::Subtract => {
            arithmetic_values(left, right, "-", |a, b| a.checked_sub(b), |a, b| a - b)
        }
        Arithmetic::Multiply => {
            arithmetic_values(left, right, "*", |a, b| a.checked_mul(b), |a, b| a * b)
        }
    }
}

fn arithmetic_values(
    left: Value,
    right: Value,
    operator: &str,
    integer_operation: impl FnOnce(i64, i64) -> Option<i64>,
    float_operation: impl FnOnce(f64, f64) -> f64,
) -> Result<Value, RuntimeError> {
    match (number(&left), number(&right)) {
        (Some(Number::Int(a)), Some(Number::Int(b))) => integer_operation(a, b)
            .map(Value::Int)
            .ok_or_else(|| RuntimeError::new(format!("Integer overflow during '{}'.", operator))),
        (Some(a), Some(b)) => checked_float(float_operation(as_float(a), as_float(b)), operator),
        _ => Err(type_error(operator, &left, Some(&right))),
    }
}

fn divide(left: Value, right: Value) -> Result<Value, RuntimeError> {
    match (number(&left), number(&right)) {
        (Some(_), Some(Number::Int(0))) => Err(RuntimeError::new("Division by zero.")),
        (Some(_), Some(Number::Float(0.0))) => Err(RuntimeError::new("Division by zero.")),
        (Some(left_number), Some(right_number)) => {
            checked_float(as_float(left_number) / as_float(right_number), "/")
        }
        _ => Err(type_error("/", &left, Some(&right))),
    }
}

fn compare(operator: BinaryOperator, left: Value, right: Value) -> Result<Value, RuntimeError> {
    let (Some(left_number), Some(right_number)) = (number(&left), number(&right)) else {
        return Err(type_error(operator_text(operator), &left, Some(&right)));
    };

    if !is_finite(left_number) || !is_finite(right_number) {
        return Err(RuntimeError::new(
            "Cannot compare non-finite numeric values.",
        ));
    }

    let ordering = compare_numbers(left_number, right_number)
        .expect("finite numeric values always have an ordering");
    let result = match operator {
        BinaryOperator::LessThan => ordering == Ordering::Less,
        BinaryOperator::GreaterThan => ordering == Ordering::Greater,
        BinaryOperator::LessEqual => ordering != Ordering::Greater,
        BinaryOperator::GreaterEqual => ordering != Ordering::Less,
        _ => unreachable!(),
    };
    Ok(Value::Bool(result))
}

fn compare_numbers(left: Number, right: Number) -> Option<Ordering> {
    match (left, right) {
        (Number::Int(left), Number::Int(right)) => Some(left.cmp(&right)),
        (Number::Float(left), Number::Float(right)) => left.partial_cmp(&right),
        (Number::Int(integer), Number::Float(float)) => compare_int_float(integer, float),
        (Number::Float(float), Number::Int(integer)) => {
            compare_int_float(integer, float).map(Ordering::reverse)
        }
    }
}

fn compare_int_float(integer: i64, float: f64) -> Option<Ordering> {
    if !float.is_finite() {
        return None;
    }

    // i64::MAX rounds up to 2^63 as f64, so use the exclusive upper bound.
    if float >= 9_223_372_036_854_775_808.0 {
        return Some(Ordering::Less);
    }
    if float < i64::MIN as f64 {
        return Some(Ordering::Greater);
    }

    let floor = float.floor();
    let integer_floor = floor as i64;
    match integer.cmp(&integer_floor) {
        Ordering::Equal if float.fract() != 0.0 => Some(Ordering::Less),
        ordering => Some(ordering),
    }
}

fn is_finite(value: Number) -> bool {
    match value {
        Number::Int(_) => true,
        Number::Float(value) => value.is_finite(),
    }
}

fn as_float(value: Number) -> f64 {
    match value {
        Number::Int(value) => value as f64,
        Number::Float(value) => value,
    }
}

fn checked_float(value: f64, operator: &str) -> Result<Value, RuntimeError> {
    if value.is_finite() {
        Ok(Value::Float(value))
    } else {
        Err(RuntimeError::new(format!(
            "Non-finite result during '{}'.",
            operator
        )))
    }
}

fn operator_text(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Add => "+",
        BinaryOperator::Subtract => "-",
        BinaryOperator::Multiply => "*",
        BinaryOperator::Divide => "/",
        BinaryOperator::Equal => "==",
        BinaryOperator::NotEqual => "!=",
        BinaryOperator::LessThan => "<",
        BinaryOperator::GreaterThan => ">",
        BinaryOperator::LessEqual => "<=",
        BinaryOperator::GreaterEqual => ">=",
        BinaryOperator::And => "and",
        BinaryOperator::Or => "or",
    }
}

fn type_error(operator: &str, left: &Value, right: Option<&Value>) -> RuntimeError {
    let message = match right {
        Some(right) => format!(
            "Operator '{}' is not supported for {} and {}.",
            operator,
            left.type_name(),
            right.type_name()
        ),
        None => format!(
            "Operator '{}' is not supported for {}.",
            operator,
            left.type_name()
        ),
    };
    RuntimeError::new(message)
}
