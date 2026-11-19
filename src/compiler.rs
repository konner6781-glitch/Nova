use std::collections::HashSet;

use crate::ast::{
    AssignmentOperator, BinaryOperator, Expression, Program, Statement, UnaryOperator,
};
use crate::bytecode::{Chunk, IndexCompoundOperator, Instruction};
use crate::value::Value;

#[derive(Default)]
pub struct Compiler {
    chunk: Chunk,
    loop_breaks: Vec<Vec<usize>>,
    function_depth: usize,
}

impl Compiler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn compile(mut self, program: &Program) -> Result<Chunk, String> {
        if !program
            .statements
            .last()
            .is_some_and(|statement| matches!(statement.unlocated(), Statement::Stop))
            || program
                .statements
                .iter()
                .filter(|statement| matches!(statement.unlocated(), Statement::Stop))
                .count()
                != 1
        {
            return Err("A compiled Nova program must end with one top-level 'stop'.".into());
        }

        let mut function_names = HashSet::new();
        let mut functions = Vec::new();
        for statement in &program.statements {
            if let Statement::Function {
                name,
                parameters,
                body,
            } = statement.unlocated()
            {
                if !function_names.insert(name.clone()) {
                    return Err(format!("Function '{}' is defined more than once.", name));
                }
                let mut parameter_names = HashSet::new();
                if parameters
                    .iter()
                    .any(|parameter| !parameter_names.insert(parameter))
                {
                    return Err(format!("Function '{}' has a duplicate parameter.", name));
                }

                let declaration = self.chunk.write(Instruction::DefineFunction {
                    name: name.clone(),
                    parameters: parameters.clone(),
                    entry_point: 0,
                    end_point: 0,
                });
                functions.push((name.clone(), parameters.clone(), body.clone(), declaration));
            }
        }

        for statement in &program.statements {
            if !matches!(statement.unlocated(), Statement::Function { .. }) {
                self.compile_statement(statement, true)?;
            }
        }

        for (name, parameters, body, declaration) in functions {
            self.function_depth = 1;
            self.loop_breaks.clear();
            let entry_point = self.chunk.instruction_count();
            for statement in &body {
                self.compile_statement(statement, false)?;
            }
            self.chunk.write(Instruction::Return { value_count: 0 });
            let end_point = self.chunk.instruction_count();
            self.chunk.patch(
                declaration,
                Instruction::DefineFunction {
                    name,
                    parameters,
                    entry_point,
                    end_point,
                },
            )?;
            self.function_depth = 0;
        }

        Ok(self.chunk)
    }

    fn compile_statement(&mut self, statement: &Statement, top_level: bool) -> Result<(), String> {
        if let Some(location) = statement.location() {
            let previous = self.chunk.current_location();
            self.chunk.set_current_location(Some(location));
            let result = self.compile_statement_inner(statement.unlocated(), top_level);
            self.chunk.set_current_location(previous);
            return result.map_err(|error| {
                if error.contains(" at line ") {
                    error
                } else {
                    format!(
                        "{} at line {}, column {}",
                        error, location.line, location.column
                    )
                }
            });
        }
        self.compile_statement_inner(statement.unlocated(), top_level)
    }

    fn compile_statement_inner(
        &mut self,
        statement: &Statement,
        top_level: bool,
    ) -> Result<(), String> {
        match statement {
            Statement::Located { .. } => {
                return Err("Nested source location wrapper in statement AST.".into());
            }
            Statement::Variable { name, value } => {
                self.compile_expression(value)?;
                self.chunk.write(Instruction::StoreVariable(name.clone()));
            }
            Statement::Assignment {
                name,
                operator,
                value,
            } => {
                if *operator != AssignmentOperator::Assign {
                    self.chunk.write(Instruction::LoadVariable(name.clone()));
                }
                self.compile_expression(value)?;
                if let Some(instruction) = assignment_instruction(*operator) {
                    self.chunk.write(instruction);
                }
                self.chunk.write(Instruction::StoreVariable(name.clone()));
            }
            Statement::IndexAssignment {
                name,
                indices,
                operator,
                value,
            } => {
                for index in indices {
                    self.compile_expression(index)?;
                }
                self.compile_expression(value)?;
                if *operator == AssignmentOperator::Assign {
                    self.chunk.write(Instruction::IndexSet {
                        name: name.clone(),
                        index_count: indices.len(),
                    });
                } else {
                    self.chunk.write(Instruction::IndexCompoundSet {
                        name: name.clone(),
                        index_count: indices.len(),
                        operator: index_compound_operator(*operator)?,
                    });
                }
            }
            Statement::UnpackAssignment { names, value } => {
                self.compile_expression(value)?;
                self.chunk.write(Instruction::Unpack(names.len()));
                for name in names {
                    self.chunk.write(Instruction::StoreVariable(name.clone()));
                }
            }
            Statement::Expression(expression) => {
                self.compile_expression(expression)?;
                self.chunk.write(Instruction::Pop);
            }
            Statement::If {
                condition,
                body,
                elif_branches,
                else_body,
            } => self.compile_if(condition, body, elif_branches, else_body.as_deref())?,
            Statement::Loop { settings, body } => self.compile_loop(settings, body)?,
            Statement::Function { name, .. } => {
                return Err(format!(
                    "Function '{}' must be declared at the top level.",
                    name
                ));
            }
            Statement::Return(values) => {
                if self.function_depth == 0 {
                    return Err("'return' can only be used inside a function.".into());
                }
                let values = values.as_deref().unwrap_or_default();
                for value in values {
                    self.compile_expression(value)?;
                }
                self.chunk.write(Instruction::Return {
                    value_count: values.len(),
                });
            }
            Statement::Break => {
                let Some(breaks) = self.loop_breaks.last_mut() else {
                    return Err("'break' can only be used inside a loop.".into());
                };
                let position = self.chunk.write(Instruction::Break(usize::MAX));
                breaks.push(position);
            }
            Statement::Import { module, item } => {
                let instruction = match item {
                    Some(item) => Instruction::ImportItem {
                        module: module.clone(),
                        item: item.clone(),
                    },
                    None => Instruction::ImportModule(module.clone()),
                };
                self.chunk.write(instruction);
            }
            Statement::Stop => {
                if !top_level || self.function_depth != 0 {
                    return Err("'stop' is only valid as the final top-level statement.".into());
                }
                self.chunk.write(Instruction::Stop);
            }
        }
        Ok(())
    }

    fn compile_if(
        &mut self,
        condition: &Expression,
        body: &[Statement],
        elif_branches: &[(Expression, Vec<Statement>)],
        else_body: Option<&[Statement]>,
    ) -> Result<(), String> {
        let mut end_jumps = Vec::new();
        self.compile_expression(condition)?;
        let mut next_branch = self.chunk.write(Instruction::JumpIfFalse(usize::MAX));
        self.compile_block(body)?;
        end_jumps.push(self.chunk.write(Instruction::Jump(usize::MAX)));
        self.patch_jump(next_branch, self.chunk.instruction_count())?;

        for (condition, body) in elif_branches {
            self.compile_expression(condition)?;
            next_branch = self.chunk.write(Instruction::JumpIfFalse(usize::MAX));
            self.compile_block(body)?;
            end_jumps.push(self.chunk.write(Instruction::Jump(usize::MAX)));
            self.patch_jump(next_branch, self.chunk.instruction_count())?;
        }

        if let Some(body) = else_body {
            self.compile_block(body)?;
        }

        let end = self.chunk.instruction_count();
        for jump in end_jumps {
            self.patch_jump(jump, end)?;
        }
        Ok(())
    }

    fn compile_loop(
        &mut self,
        settings: &crate::ast::LoopSettings,
        body: &[Statement],
    ) -> Result<(), String> {
        self.chunk.write(Instruction::LoopStart);
        if let Some(maximum) = &settings.max_iterations {
            self.compile_expression(maximum)?;
            self.chunk.write(Instruction::LoopSetLimit);
        }
        if let Some(delay) = &settings.delay {
            self.compile_expression(delay)?;
            self.chunk.write(Instruction::LoopSetDelay);
        }

        let condition_start = self.chunk.instruction_count();
        let limit_check = if settings.max_iterations.is_some() {
            Some(self.chunk.write(Instruction::LoopLimitCheck(usize::MAX)))
        } else {
            None
        };
        if let Some(condition) = &settings.condition {
            self.compile_expression(condition)?;
        } else {
            self.emit_constant(Value::Bool(true));
        }
        let check = self.chunk.write(Instruction::LoopCheck(usize::MAX));

        self.loop_breaks.push(Vec::new());
        self.compile_block(body)?;
        let breaks = self.loop_breaks.pop().expect("loop context must exist");
        self.chunk.write(Instruction::LoopEnd(condition_start));

        let end = self.chunk.instruction_count();
        if let Some(limit_check) = limit_check {
            self.patch_jump(limit_check, end)?;
        }
        self.patch_jump(check, end)?;
        for break_instruction in breaks {
            self.patch_break(break_instruction, end)?;
        }
        Ok(())
    }

    fn compile_block(&mut self, statements: &[Statement]) -> Result<(), String> {
        for statement in statements {
            self.compile_statement(statement, false)?;
        }
        Ok(())
    }

    fn compile_expression(&mut self, expression: &Expression) -> Result<(), String> {
        if let Expression::Located {
            location,
            expression,
        } = expression
        {
            let previous = self.chunk.current_location();
            self.chunk.set_current_location(Some(*location));
            let result = self.compile_expression(expression);
            self.chunk.set_current_location(previous);
            return result;
        }
        self.compile_expression_inner(expression)
    }

    fn compile_expression_inner(&mut self, expression: &Expression) -> Result<(), String> {
        match expression {
            Expression::Located { .. } => {
                return Err("Nested source location wrapper in expression AST.".into());
            }
            Expression::Integer(value) => self.emit_constant(Value::Int(*value)),
            Expression::Float(value) if value.is_finite() => {
                self.emit_constant(Value::Float(*value))
            }
            Expression::Float(_) => return Err("Float literal must be finite.".into()),
            Expression::String(value) => self.emit_constant(Value::String(value.clone())),
            Expression::Boolean(value) => self.emit_constant(Value::Bool(*value)),
            Expression::Identifier(name) => {
                self.chunk.write(Instruction::LoadVariable(name.clone()));
            }
            Expression::Grouped(expression) => self.compile_expression(expression)?,
            Expression::List(elements) => {
                for element in elements {
                    self.compile_expression(element)?;
                }
                self.chunk.write(Instruction::BuildList(elements.len()));
            }
            Expression::Dictionary(entries) => {
                for (key, value) in entries {
                    self.compile_expression(key)?;
                    self.compile_expression(value)?;
                }
                self.chunk
                    .write(Instruction::BuildDictionary(entries.len()));
            }
            Expression::Index { object, index } => {
                self.compile_expression(object)?;
                self.compile_expression(index)?;
                self.chunk.write(Instruction::IndexGet);
            }
            Expression::Member { object, name } => {
                self.compile_expression(object)?;
                self.emit_constant(Value::String(name.clone()));
                self.chunk.write(Instruction::IndexGet);
            }
            Expression::Unary { operator, operand } => {
                self.compile_expression(operand)?;
                self.chunk.write(match operator {
                    UnaryOperator::Negative => Instruction::Negate,
                    UnaryOperator::Not => Instruction::Not,
                });
            }
            Expression::Binary {
                left,
                operator: BinaryOperator::And,
                right,
            } => self.compile_logical(left, right, false)?,
            Expression::Binary {
                left,
                operator: BinaryOperator::Or,
                right,
            } => self.compile_logical(left, right, true)?,
            Expression::Binary {
                left,
                operator,
                right,
            } => {
                self.compile_expression(left)?;
                self.compile_expression(right)?;
                self.chunk.write(binary_instruction(*operator)?);
            }
            Expression::Call { name, arguments } if name == "print" => {
                for argument in arguments {
                    self.compile_expression(argument)?;
                }
                self.chunk.write(Instruction::Print {
                    argument_count: arguments.len(),
                });
            }
            Expression::Call { name, arguments } => {
                for argument in arguments {
                    self.compile_expression(argument)?;
                }
                self.chunk.write(Instruction::CallFunction {
                    name: name.clone(),
                    argument_count: arguments.len(),
                });
            }
        }
        Ok(())
    }

    fn compile_logical(
        &mut self,
        left: &Expression,
        right: &Expression,
        is_or: bool,
    ) -> Result<(), String> {
        self.compile_expression(left)?;
        let branch = if is_or {
            self.chunk.write(Instruction::JumpIfTrue(usize::MAX))
        } else {
            self.chunk.write(Instruction::JumpIfFalse(usize::MAX))
        };
        self.compile_expression(right)?;
        self.chunk.write(Instruction::Not);
        self.chunk.write(Instruction::Not);
        let end_jump = self.chunk.write(Instruction::Jump(usize::MAX));

        self.patch_jump(branch, self.chunk.instruction_count())?;
        self.emit_constant(Value::Bool(is_or));
        self.patch_jump(end_jump, self.chunk.instruction_count())?;
        Ok(())
    }

    fn emit_constant(&mut self, value: Value) {
        let constant = self.chunk.add_constant(value);
        self.chunk.write(Instruction::LoadConstant(constant));
    }

    fn patch_jump(&mut self, position: usize, target: usize) -> Result<(), String> {
        let replacement = match self.chunk.get_instruction(position) {
            Some(Instruction::Jump(_)) => Instruction::Jump(target),
            Some(Instruction::JumpIfFalse(_)) => Instruction::JumpIfFalse(target),
            Some(Instruction::JumpIfTrue(_)) => Instruction::JumpIfTrue(target),
            Some(Instruction::LoopLimitCheck(_)) => Instruction::LoopLimitCheck(target),
            Some(Instruction::LoopCheck(_)) => Instruction::LoopCheck(target),
            Some(instruction) => {
                return Err(format!(
                    "Instruction at {} is not a patchable jump: {:?}.",
                    position, instruction
                ));
            }
            None => return Err(format!("Missing jump instruction at {}.", position)),
        };
        self.chunk.patch(position, replacement)
    }

    fn patch_break(&mut self, position: usize, target: usize) -> Result<(), String> {
        self.chunk.patch(position, Instruction::Break(target))
    }
}

pub fn compile(program: &Program) -> Result<Chunk, String> {
    Compiler::new().compile(program)
}

fn assignment_instruction(operator: AssignmentOperator) -> Option<Instruction> {
    match operator {
        AssignmentOperator::Assign => None,
        AssignmentOperator::Add => Some(Instruction::Add),
        AssignmentOperator::Subtract => Some(Instruction::Subtract),
        AssignmentOperator::Multiply => Some(Instruction::Multiply),
        AssignmentOperator::Divide => Some(Instruction::Divide),
    }
}

fn index_compound_operator(operator: AssignmentOperator) -> Result<IndexCompoundOperator, String> {
    match operator {
        AssignmentOperator::Add => Ok(IndexCompoundOperator::Add),
        AssignmentOperator::Subtract => Ok(IndexCompoundOperator::Subtract),
        AssignmentOperator::Multiply => Ok(IndexCompoundOperator::Multiply),
        AssignmentOperator::Divide => Ok(IndexCompoundOperator::Divide),
        AssignmentOperator::Assign => Err("Plain assignment has no compound operator.".into()),
    }
}

fn binary_instruction(operator: BinaryOperator) -> Result<Instruction, String> {
    Ok(match operator {
        BinaryOperator::Add => Instruction::Add,
        BinaryOperator::Subtract => Instruction::Subtract,
        BinaryOperator::Multiply => Instruction::Multiply,
        BinaryOperator::Divide => Instruction::Divide,
        BinaryOperator::Equal => Instruction::Equal,
        BinaryOperator::NotEqual => Instruction::NotEqual,
        BinaryOperator::LessThan => Instruction::LessThan,
        BinaryOperator::GreaterThan => Instruction::GreaterThan,
        BinaryOperator::LessEqual => Instruction::LessThanOrEqual,
        BinaryOperator::GreaterEqual => Instruction::GreaterThanOrEqual,
        BinaryOperator::And | BinaryOperator::Or => {
            return Err("Logical operators must use short-circuit compilation.".into());
        }
    })
}
