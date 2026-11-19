use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use crate::ast::{BinaryOperator, UnaryOperator};
use crate::bytecode::{Chunk, IndexCompoundOperator, Instruction};
use crate::runtime::{apply_binary, apply_unary};
use crate::tokens::SourceLocation;
use crate::ui::{UiCallback, UiRuntime};
use crate::value::{NativeFunction, Value, normalize_dictionary};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmError {
    message: String,
    location: Option<SourceLocation>,
}

impl VmError {
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

    pub fn location(&self) -> Option<SourceLocation> {
        self.location
    }
}

impl fmt::Display for VmError {
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

impl std::error::Error for VmError {}

#[derive(Debug, Clone)]
struct Function {
    parameters: Vec<String>,
    entry_point: usize,
    end_point: usize,
}

#[derive(Debug)]
struct CallFrame {
    return_ip: usize,
    stack_base: usize,
    loop_base: usize,
    end_point: usize,
    ui_callback: bool,
}

#[derive(Debug)]
struct LoopFrame {
    iterations: u64,
    max_iterations: Option<u64>,
    delay: Duration,
}

/// Executes Nova bytecode with a value stack, global variables, function
/// frames, and independent loop frames.
pub struct Vm {
    chunk: Chunk,
    ip: usize,
    stack: Vec<Value>,
    globals: HashMap<String, Value>,
    locals: Vec<HashMap<String, Value>>,
    functions: HashMap<String, Function>,
    call_frames: Vec<CallFrame>,
    loops: Vec<LoopFrame>,
    output: Vec<String>,
    modules: HashMap<String, HashMap<String, Value>>,
    ui: UiRuntime,
    halted: bool,
}

impl Vm {
    pub fn new(chunk: Chunk) -> Self {
        Self {
            chunk,
            ip: 0,
            stack: Vec::new(),
            globals: HashMap::new(),
            locals: Vec::new(),
            functions: HashMap::new(),
            call_frames: Vec::new(),
            loops: Vec::new(),
            output: Vec::new(),
            modules: crate::stdlib::builtin_modules(),
            ui: UiRuntime::new(),
            halted: false,
        }
    }

    /// Registers values that a Nova import statement can expose.
    ///
    /// Importing a module binds a dictionary under its module name. Importing
    /// one item binds that value directly under the requested item name.
    pub fn register_module(&mut self, name: impl Into<String>, exports: HashMap<String, Value>) {
        self.modules.insert(name.into(), exports);
    }

    pub fn globals(&self) -> &HashMap<String, Value> {
        &self.globals
    }

    pub fn output(&self) -> &[String] {
        &self.output
    }

    pub fn take_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.output)
    }

    pub fn run(&mut self) -> Result<(), VmError> {
        loop {
            while !self.halted {
                if let Some(frame) = self.call_frames.last() {
                    if self.ip >= frame.end_point {
                        return Err(VmError::new(
                            "Function reached its end without executing a return.",
                        )
                        .with_location(self.chunk.source_location(self.ip.saturating_sub(1))));
                    }
                }

                let instruction_ip = self.ip;
                let instruction = self
                    .chunk
                    .get_instruction(instruction_ip)
                    .cloned()
                    .ok_or_else(|| {
                        VmError::new("Bytecode ended without a stop instruction.").with_location(
                            self.chunk.source_location(instruction_ip.saturating_sub(1)),
                        )
                    })?;
                self.ip += 1;
                self.execute(instruction).map_err(|error| {
                    error.with_location(self.chunk.source_location(instruction_ip))
                })?;
            }

            match self.ui.wait_for_callback().map_err(VmError::new)? {
                Some(callback) => self.start_ui_callback(callback)?,
                None => return Ok(()),
            }
        }
    }

    fn start_ui_callback(&mut self, callback: UiCallback) -> Result<(), VmError> {
        let function = self.functions.get(&callback.name).cloned().ok_or_else(|| {
            VmError::new(format!(
                "UI callback '{}' is no longer available.",
                callback.name
            ))
        })?;
        if function.parameters.len() != callback.arguments.len() {
            return Err(VmError::new(format!(
                "UI callback '{}' expects {} arguments but received {}.",
                callback.name,
                function.parameters.len(),
                callback.arguments.len()
            )));
        }
        let stack_base = self.stack.len();
        let locals = function
            .parameters
            .iter()
            .cloned()
            .zip(callback.arguments)
            .collect();
        self.call_frames.push(CallFrame {
            return_ip: self.ip,
            stack_base,
            loop_base: self.loops.len(),
            end_point: function.end_point,
            ui_callback: true,
        });
        self.locals.push(locals);
        self.ip = function.entry_point;
        self.halted = false;
        Ok(())
    }

    fn execute(&mut self, instruction: Instruction) -> Result<(), VmError> {
        match instruction {
            Instruction::LoadConstant(index) => {
                let value =
                    self.chunk.get_constant(index).cloned().ok_or_else(|| {
                        VmError::new(format!("Invalid constant index {}.", index))
                    })?;
                value.validate_finite().map_err(VmError::new)?;
                self.stack.push(value);
            }
            Instruction::LoadNone => self.stack.push(Value::None),
            Instruction::DefineVariable(name) | Instruction::StoreVariable(name) => {
                let value = self.pop("store variable")?;
                self.store_variable(name, value);
            }
            Instruction::LoadVariable(name) => {
                let value = self.load_variable(&name)?;
                self.stack.push(value);
            }
            Instruction::DeleteVariable(name) => {
                self.delete_variable(&name)?;
            }
            Instruction::Add => self.binary(BinaryOperator::Add)?,
            Instruction::Subtract => self.binary(BinaryOperator::Subtract)?,
            Instruction::Multiply => self.binary(BinaryOperator::Multiply)?,
            Instruction::Divide => self.binary(BinaryOperator::Divide)?,
            Instruction::Modulo => self.modulo()?,
            Instruction::Power => self.power()?,
            Instruction::Negate => self.unary(UnaryOperator::Negative)?,
            Instruction::Equal => self.binary(BinaryOperator::Equal)?,
            Instruction::NotEqual => self.binary(BinaryOperator::NotEqual)?,
            Instruction::GreaterThan => self.binary(BinaryOperator::GreaterThan)?,
            Instruction::LessThan => self.binary(BinaryOperator::LessThan)?,
            Instruction::GreaterThanOrEqual => self.binary(BinaryOperator::GreaterEqual)?,
            Instruction::LessThanOrEqual => self.binary(BinaryOperator::LessEqual)?,
            Instruction::And => self.binary(BinaryOperator::And)?,
            Instruction::Or => self.binary(BinaryOperator::Or)?,
            Instruction::Not => self.unary(UnaryOperator::Not)?,
            Instruction::Jump(target) => self.jump(target)?,
            Instruction::JumpIfFalse(target) => {
                if !self.pop_truthy("conditional jump")? {
                    self.jump(target)?;
                }
            }
            Instruction::JumpIfTrue(target) => {
                if self.pop_truthy("conditional jump")? {
                    self.jump(target)?;
                }
            }
            Instruction::LoopStart => self.loops.push(LoopFrame {
                iterations: 0,
                max_iterations: None,
                delay: Duration::ZERO,
            }),
            Instruction::LoopSetLimit => {
                let value = self.pop("set loop iteration limit")?;
                let limit = iteration_count(&value, "Loop iteration limit")?;
                self.current_loop_mut()?.max_iterations = Some(limit);
            }
            Instruction::LoopSetDelay => {
                let value = self.pop("set loop delay")?;
                let seconds = numeric_seconds(&value, "loop delay")?;
                let delay = Duration::try_from_secs_f64(seconds)
                    .map_err(|_| VmError::new("Loop delay is too large."))?;
                self.current_loop_mut()?.delay = delay;
            }
            Instruction::LoopLimitCheck(target) => {
                let frame = self
                    .loops
                    .last()
                    .ok_or_else(|| VmError::new("Loop limit check has no active loop."))?;
                if frame
                    .max_iterations
                    .is_some_and(|limit| frame.iterations >= limit)
                {
                    self.loops.pop();
                    self.jump(target)?;
                }
            }
            Instruction::LoopCheck(target) => {
                let value = self.pop("check loop condition")?;
                let frame = self
                    .loops
                    .last()
                    .ok_or_else(|| VmError::new("Loop check has no active loop."))?;
                let keep_running = repeat_value(&value, frame.iterations)?;
                if !keep_running {
                    self.loops.pop();
                    self.jump(target)?;
                } else if frame.iterations > 0 && !frame.delay.is_zero() {
                    std::thread::sleep(frame.delay);
                }
            }
            Instruction::LoopEnd(target) => {
                let frame = self.current_loop_mut()?;
                frame.iterations = frame
                    .iterations
                    .checked_add(1)
                    .ok_or_else(|| VmError::new("Loop iteration count overflow."))?;
                self.jump(target)?;
            }
            Instruction::Break(target) => {
                let loop_base = self.call_frames.last().map_or(0, |frame| frame.loop_base);
                if self.loops.len() <= loop_base {
                    return Err(VmError::new("'break' has no active loop in this function."));
                }
                self.loops.pop();
                self.jump(target)?;
            }
            Instruction::DefineFunction {
                name,
                parameters,
                entry_point,
                end_point,
            } => {
                if entry_point >= end_point || end_point > self.chunk.instruction_count() {
                    return Err(VmError::new(format!(
                        "Function '{}' has invalid bytecode boundaries.",
                        name
                    )));
                }
                self.functions.insert(
                    name,
                    Function {
                        parameters,
                        entry_point,
                        end_point,
                    },
                );
            }
            Instruction::CallFunction {
                name,
                argument_count,
            } => self.call_function(&name, argument_count)?,
            Instruction::Return { value_count } => self.return_from_function(value_count)?,
            Instruction::BuildList(count) => {
                let values = self.pop_many(count, "build list")?;
                self.stack.push(Value::List(values));
            }
            Instruction::BuildDictionary(count) => {
                let values = self.pop_many(count.saturating_mul(2), "build dictionary")?;
                let mut entries: Vec<(Value, Value)> = Vec::with_capacity(count);
                for pair in values.chunks_exact(2) {
                    if let Some((_, existing_value)) =
                        entries.iter_mut().find(|(key, _)| key == &pair[0])
                    {
                        *existing_value = pair[1].clone();
                    } else {
                        entries.push((pair[0].clone(), pair[1].clone()));
                    }
                }
                self.stack.push(Value::Dictionary(entries));
            }
            Instruction::IndexGet => {
                let index = self.pop("index collection")?;
                let collection = self.pop("index collection")?;
                self.stack.push(read_index(&collection, &index)?);
            }
            Instruction::IndexSet { name, index_count } => {
                if index_count == 0 {
                    return Err(VmError::new(
                        "Index assignment requires at least one index.",
                    ));
                }
                let value = self.pop("set collection index")?;
                let indices = self.pop_many(index_count, "set collection index")?;
                let collection = self.load_variable(&name)?;
                let collection = write_index_path(collection, &indices, value)?;
                self.store_variable(name, collection);
            }
            Instruction::IndexCompoundSet {
                name,
                index_count,
                operator,
            } => {
                if index_count == 0 {
                    return Err(VmError::new(
                        "Index assignment requires at least one index.",
                    ));
                }
                let right = self.pop("update collection index")?;
                let indices = self.pop_many(index_count, "update collection index")?;
                let collection = self.load_variable(&name)?;
                let old_value = read_index_path(&collection, &indices)?;
                let operator = match operator {
                    IndexCompoundOperator::Add => BinaryOperator::Add,
                    IndexCompoundOperator::Subtract => BinaryOperator::Subtract,
                    IndexCompoundOperator::Multiply => BinaryOperator::Multiply,
                    IndexCompoundOperator::Divide => BinaryOperator::Divide,
                };
                let value = apply_binary(operator, old_value, right)
                    .map_err(|error| VmError::new(error.to_string()))?;
                let collection = write_index_path(collection, &indices, value)?;
                self.store_variable(name, collection);
            }
            Instruction::Unpack(count) => {
                let value = self.pop("unpack values")?;
                let Value::List(values) = value else {
                    return Err(VmError::new(
                        "Multiple assignment requires a list or multiple return value.",
                    ));
                };
                if values.len() != count {
                    return Err(VmError::new(format!(
                        "Cannot unpack {} values into {} variables.",
                        values.len(),
                        count
                    )));
                }
                self.stack.extend(values.into_iter().rev());
            }
            Instruction::Pop => {
                self.pop("pop")?;
            }
            Instruction::Duplicate => {
                let value = self
                    .stack
                    .last()
                    .cloned()
                    .ok_or_else(|| VmError::new("Operand stack underflow during duplicate."))?;
                self.stack.push(value);
            }
            Instruction::Swap => {
                if self.stack.len() < 2 {
                    return Err(VmError::new("Operand stack underflow during swap."));
                }
                let length = self.stack.len();
                self.stack.swap(length - 1, length - 2);
            }
            Instruction::Print { argument_count } => {
                let arguments = self.pop_many(argument_count, "print")?;
                self.output.push(
                    arguments
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(" "),
                );
                self.stack.push(Value::None);
            }
            Instruction::ImportModule(name) => {
                let exports = self
                    .modules
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| VmError::new(format!("Unknown module '{}'.", name)))?;
                for value in exports.values() {
                    value.validate_finite().map_err(VmError::new)?;
                }
                let mut exports: Vec<_> = exports.into_iter().collect();
                exports.sort_by(|(left, _), (right, _)| left.cmp(right));
                let dictionary = exports
                    .into_iter()
                    .map(|(key, value)| (Value::String(key), value))
                    .collect();
                self.store_variable(name, Value::Dictionary(dictionary));
            }
            Instruction::ImportItem { module, item } => {
                let value = self
                    .modules
                    .get(&module)
                    .and_then(|exports| exports.get(&item))
                    .cloned()
                    .ok_or_else(|| {
                        VmError::new(format!("Module '{}' has no item '{}'.", module, item))
                    })?;
                value.validate_finite().map_err(VmError::new)?;
                self.store_variable(item, value);
            }
            Instruction::Stop => self.halted = true,
        }
        Ok(())
    }

    fn binary(&mut self, operator: BinaryOperator) -> Result<(), VmError> {
        let right = self.pop("binary operation")?;
        let left = self.pop("binary operation")?;
        let result =
            apply_binary(operator, left, right).map_err(|error| VmError::new(error.to_string()))?;
        self.stack.push(result);
        Ok(())
    }

    fn unary(&mut self, operator: UnaryOperator) -> Result<(), VmError> {
        let value = self.pop("unary operation")?;
        let result =
            apply_unary(operator, value).map_err(|error| VmError::new(error.to_string()))?;
        self.stack.push(result);
        Ok(())
    }

    fn modulo(&mut self) -> Result<(), VmError> {
        let right = self.pop("modulo")?;
        let left = self.pop("modulo")?;
        let result = match (&left, &right) {
            (Value::Int(_), Value::Int(0)) => return Err(VmError::new("Modulo by zero.")),
            (Value::Int(a), Value::Int(b)) => Value::Int(
                a.checked_rem(*b)
                    .ok_or_else(|| VmError::new("Integer overflow during modulo."))?,
            ),
            (Value::Int(_), Value::Float(b)) if *b == 0.0 => {
                return Err(VmError::new("Modulo by zero."));
            }
            (Value::Float(a), Value::Int(0)) => {
                let _ = a;
                return Err(VmError::new("Modulo by zero."));
            }
            (Value::Float(_), Value::Float(b)) if *b == 0.0 => {
                return Err(VmError::new("Modulo by zero."));
            }
            (Value::Int(a), Value::Float(b)) => checked_float((*a as f64) % b, "%")?,
            (Value::Float(a), Value::Int(b)) => checked_float(a % (*b as f64), "%")?,
            (Value::Float(a), Value::Float(b)) => checked_float(a % b, "%")?,
            _ => {
                return Err(VmError::new(format!(
                    "Operator '%' is not supported for {} and {}.",
                    left.type_name(),
                    right.type_name()
                )));
            }
        };
        self.stack.push(result);
        Ok(())
    }

    fn power(&mut self) -> Result<(), VmError> {
        let right = self.pop("power")?;
        let left = self.pop("power")?;
        let result = match (&left, &right) {
            (Value::Int(a), Value::Int(b)) if *b >= 0 && *b <= u32::MAX as i64 => Value::Int(
                a.checked_pow(*b as u32)
                    .ok_or_else(|| VmError::new("Integer overflow during power operation."))?,
            ),
            (Value::Int(a), Value::Int(b)) => checked_float((*a as f64).powf(*b as f64), "^")?,
            (Value::Int(a), Value::Float(b)) => checked_float((*a as f64).powf(*b), "^")?,
            (Value::Float(a), Value::Int(b)) => checked_float(a.powf(*b as f64), "^")?,
            (Value::Float(a), Value::Float(b)) => checked_float(a.powf(*b), "^")?,
            _ => {
                return Err(VmError::new(format!(
                    "Operator '^' is not supported for {} and {}.",
                    left.type_name(),
                    right.type_name()
                )));
            }
        };
        self.stack.push(result);
        Ok(())
    }

    fn call_function(&mut self, name: &str, argument_count: usize) -> Result<(), VmError> {
        let Some(function) = self.functions.get(name).cloned() else {
            if let Some(native) = self.resolve_native_function(name) {
                let arguments = self.pop_many(argument_count, "native function call")?;
                let value = if native.module == "UI" {
                    let arities: HashMap<_, _> = self
                        .functions
                        .iter()
                        .map(|(name, function)| (name.clone(), function.parameters.len()))
                        .collect();
                    self.ui
                        .call(&native, &arguments, |callback| {
                            arities.get(callback).copied()
                        })
                        .map_err(VmError::new)?
                } else {
                    crate::stdlib::call(&native, &arguments).map_err(VmError::new)?
                };
                self.stack.push(value);
                return Ok(());
            }
            return self.call_builtin(name, argument_count);
        };
        if function.parameters.len() != argument_count {
            return Err(VmError::new(format!(
                "Function '{}' expects {} arguments but received {}.",
                name,
                function.parameters.len(),
                argument_count
            )));
        }

        let arguments = self.pop_many(argument_count, "function call")?;
        let stack_base = self.stack.len();
        let locals = function.parameters.iter().cloned().zip(arguments).collect();
        self.call_frames.push(CallFrame {
            return_ip: self.ip,
            stack_base,
            loop_base: self.loops.len(),
            end_point: function.end_point,
            ui_callback: false,
        });
        self.locals.push(locals);
        self.ip = function.entry_point;
        Ok(())
    }

    fn resolve_native_function(&self, name: &str) -> Option<NativeFunction> {
        let value = self.load_variable(name).ok();
        if let Some(Value::NativeFunction(function)) = value {
            return Some(function);
        }

        let (module_name, item_name) = name.split_once('.')?;
        let Value::Dictionary(exports) = self.load_variable(module_name).ok()? else {
            return None;
        };
        exports.into_iter().find_map(|(key, value)| {
            if key == Value::String(item_name.to_string()) {
                if let Value::NativeFunction(function) = value {
                    return Some(function);
                }
            }
            None
        })
    }

    fn call_builtin(&mut self, name: &str, argument_count: usize) -> Result<(), VmError> {
        if !matches!(name, "int" | "float" | "str" | "bool") {
            return Err(VmError::new(format!("Undefined function '{}'.", name)));
        }
        if argument_count != 1 {
            return Err(VmError::new(format!(
                "Function '{}' expects 1 argument but received {}.",
                name, argument_count
            )));
        }

        let value = self.pop("conversion function")?;
        let converted = match name {
            "int" => Value::Int(value.to_int().map_err(VmError::new)?),
            "float" => Value::Float(value.to_float().map_err(VmError::new)?),
            "str" => Value::String(value.to_string_value()),
            "bool" => Value::Bool(value.to_bool().map_err(VmError::new)?),
            _ => unreachable!(),
        };
        self.stack.push(converted);
        Ok(())
    }

    fn return_from_function(&mut self, value_count: usize) -> Result<(), VmError> {
        let values = self.pop_many(value_count, "return")?;
        let frame = self
            .call_frames
            .pop()
            .ok_or_else(|| VmError::new("'return' executed outside a function."))?;
        self.locals
            .pop()
            .ok_or_else(|| VmError::new("Function local scope is missing."))?;
        self.stack.truncate(frame.stack_base);
        let value = match values.len() {
            0 => Value::None,
            1 => values.into_iter().next().unwrap(),
            _ => Value::List(values),
        };
        if !frame.ui_callback {
            self.stack.push(value);
        }
        self.loops.truncate(frame.loop_base);
        self.ip = frame.return_ip;
        if frame.ui_callback {
            self.halted = true;
        }
        Ok(())
    }

    fn pop(&mut self, operation: &str) -> Result<Value, VmError> {
        self.stack
            .pop()
            .ok_or_else(|| VmError::new(format!("Operand stack underflow during {}.", operation)))
    }

    fn pop_many(&mut self, count: usize, operation: &str) -> Result<Vec<Value>, VmError> {
        if self.stack.len() < count {
            return Err(VmError::new(format!(
                "Operand stack underflow during {}.",
                operation
            )));
        }
        let start = self.stack.len() - count;
        Ok(self.stack.split_off(start))
    }

    fn pop_truthy(&mut self, operation: &str) -> Result<bool, VmError> {
        self.pop(operation)?.to_bool().map_err(VmError::new)
    }

    fn jump(&mut self, target: usize) -> Result<(), VmError> {
        if target >= self.chunk.instruction_count() {
            return Err(VmError::new(format!(
                "Jump target {} is outside the bytecode chunk.",
                target
            )));
        }
        self.ip = target;
        Ok(())
    }

    fn store_variable(&mut self, name: String, value: Value) {
        if let Some(locals) = self.locals.last_mut() {
            locals.insert(name, value);
        } else {
            self.globals.insert(name, value);
        }
    }

    fn load_variable(&self, name: &str) -> Result<Value, VmError> {
        let value = if let Some(locals) = self.locals.last() {
            locals.get(name)
        } else {
            self.globals.get(name)
        };
        value
            .cloned()
            .ok_or_else(|| VmError::new(format!("Undefined variable '{}'.", name)))
    }

    fn delete_variable(&mut self, name: &str) -> Result<(), VmError> {
        let removed = if let Some(locals) = self.locals.last_mut() {
            locals.remove(name).is_some()
        } else {
            self.globals.remove(name).is_some()
        };
        if removed {
            Ok(())
        } else {
            Err(VmError::new(format!("Undefined variable '{}'.", name)))
        }
    }

    fn current_loop_mut(&mut self) -> Result<&mut LoopFrame, VmError> {
        self.loops
            .last_mut()
            .ok_or_else(|| VmError::new("Loop instruction has no active loop."))
    }
}

fn read_index_path(collection: &Value, indices: &[Value]) -> Result<Value, VmError> {
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
) -> Result<Value, VmError> {
    let Some((index, remainder)) = indices.split_first() else {
        return Err(VmError::new(
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

fn read_index(collection: &Value, index: &Value) -> Result<Value, VmError> {
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
            .ok_or_else(|| VmError::new(format!("Dictionary has no key '{}'.", index))),
        value => Err(VmError::new(format!(
            "Cannot index a {} value.",
            value.type_name()
        ))),
    }
}

fn write_index(collection: Value, index: &Value, replacement: Value) -> Result<Value, VmError> {
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
        value => Err(VmError::new(format!(
            "Cannot assign an index of a {} value.",
            value.type_name()
        ))),
    }
}

fn list_position(index: &Value, length: usize) -> Result<usize, VmError> {
    let Value::Int(index) = index else {
        return Err(VmError::new(format!(
            "List indices must be Int values, received {}.",
            index.type_name()
        )));
    };
    if *index < 0 || *index as u128 >= length as u128 {
        return Err(VmError::new(format!(
            "List index {} is out of range for length {}.",
            index, length
        )));
    }
    Ok(*index as usize)
}

fn repeat_value(value: &Value, iterations: u64) -> Result<bool, VmError> {
    match value {
        Value::Bool(condition) => Ok(*condition),
        Value::Int(count) if *count >= 0 => Ok(iterations < *count as u64),
        Value::Int(_) => Err(VmError::new("Repeat count cannot be negative.")),
        Value::Float(count) => {
            let count = iteration_count(&Value::Float(*count), "Repeat count")?;
            Ok(iterations < count)
        }
        _ => Err(VmError::new(format!(
            "Repeat expression must be a number or Boolean, received {}.",
            value.type_name()
        ))),
    }
}

fn iteration_count(value: &Value, description: &str) -> Result<u64, VmError> {
    match value {
        Value::Int(count) if *count >= 0 => Ok(*count as u64),
        Value::Int(_) => Err(VmError::new(format!("{} cannot be negative.", description))),
        Value::Float(count)
            if count.is_finite()
                && *count >= 0.0
                && *count < 18_446_744_073_709_551_616.0
                && count.fract() == 0.0 =>
        {
            Ok(*count as u64)
        }
        Value::Float(_) => Err(VmError::new(format!(
            "{} must be a non-negative whole number.",
            description
        ))),
        _ => Err(VmError::new(format!(
            "{} must be numeric, received {}.",
            description,
            value.type_name()
        ))),
    }
}

fn numeric_seconds(value: &Value, description: &str) -> Result<f64, VmError> {
    let seconds = match value {
        Value::Int(value) => *value as f64,
        Value::Float(value) => *value,
        _ => {
            return Err(VmError::new(format!(
                "{} must be numeric, received {}.",
                description,
                value.type_name()
            )));
        }
    };
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(VmError::new(format!(
            "{} must be a finite non-negative number of seconds.",
            description
        )));
    }
    Ok(seconds)
}

fn checked_float(value: f64, operator: &str) -> Result<Value, VmError> {
    if value.is_finite() {
        Ok(Value::Float(value))
    } else {
        Err(VmError::new(format!(
            "Non-finite result during '{}'.",
            operator
        )))
    }
}

#[cfg(test)]
mod ui_callback_tests {
    use super::*;
    use crate::compiler;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    #[test]
    fn ui_callback_runs_on_the_same_vm_and_applies_live_updates() {
        let source = "get UI\nfunction update(field):\n    get UI\n    UI.set_value(field, \"updated\")\nfield = UI.input(UI.window(\"Test\", 640, 420), \"before\")\nstop";
        let tokens = Lexer::new(source).tokenize().unwrap();
        let program = Parser::new(tokens).parse().unwrap();
        let chunk = compiler::compile(&program).unwrap();
        let mut vm = Vm::new(chunk);
        vm.run().unwrap();
        let field = vm.globals().get("field").unwrap().clone();

        vm.start_ui_callback(UiCallback {
            name: "update".into(),
            arguments: vec![field.clone()],
        })
        .unwrap();
        while !vm.halted {
            let instruction = vm.chunk.get_instruction(vm.ip).unwrap().clone();
            vm.ip += 1;
            vm.execute(instruction).unwrap();
        }

        let updated = vm
            .ui
            .call(&NativeFunction::new("UI", "value"), &[field], |_| None)
            .unwrap();
        assert_eq!(updated, Value::String("updated".into()));
        assert!(
            vm.stack.is_empty(),
            "callback return value must be discarded"
        );
    }
}
