use std::fmt;

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    List(Vec<Value>),
    Dictionary(Vec<(Value, Value)>),
    NativeFunction(NativeFunction),
    UiWindow(u64),
    UiComponent(u64),
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFunction {
    pub module: String,
    pub name: String,
}

impl NativeFunction {
    pub fn new(module: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            name: name.into(),
        }
    }
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::String(_) => "String",
            Value::Bool(_) => "Bool",
            Value::List(_) => "List",
            Value::Dictionary(_) => "Dictionary",
            Value::NativeFunction(_) => "Function",
            Value::UiWindow(_) => "Window",
            Value::UiComponent(_) => "Component",
            Value::None => "None",
        }
    }

    pub fn is_int(&self) -> bool {
        matches!(self, Value::Int(_))
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Value::Float(_))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Value::String(_))
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Value::Bool(_))
    }

    pub fn is_list(&self) -> bool {
        matches!(self, Value::List(_))
    }

    pub fn is_dictionary(&self) -> bool {
        matches!(self, Value::Dictionary(_))
    }

    /// Rejects non-finite floats, including values nested in collections.
    pub fn validate_finite(&self) -> Result<(), String> {
        match self {
            Value::Float(value) if !value.is_finite() => {
                Err("Non-finite Float values are not supported.".into())
            }
            Value::List(values) => values.iter().try_for_each(Value::validate_finite),
            Value::Dictionary(entries) => entries.iter().try_for_each(|(key, value)| {
                key.validate_finite()?;
                value.validate_finite()
            }),
            _ => Ok(()),
        }
    }
}

impl Value {
    /// Converts a value into an integer.
    pub fn to_int(&self) -> Result<i64, String> {
        match self {
            Value::Int(value) => Ok(*value),
            Value::Float(value) => {
                if value.is_finite() && *value >= i64::MIN as f64 && *value < 9223372036854775808.0
                {
                    Ok(*value as i64)
                } else {
                    Err("Cannot convert Float to Int".to_string())
                }
            }
            Value::String(value) => value
                .parse::<i64>()
                .map_err(|_| format!("Cannot convert '{}' to Int", value)),
            Value::Bool(value) => Ok(if *value { 1 } else { 0 }),
            Value::List(_) => Err("Cannot convert List to Int".to_string()),
            Value::Dictionary(_) => Err("Cannot convert Dictionary to Int".to_string()),
            Value::NativeFunction(_) => Err("Cannot convert Function to Int".to_string()),
            Value::UiWindow(_) | Value::UiComponent(_) => {
                Err("Cannot convert UI handle to Int".to_string())
            }
            Value::None => Err("Cannot convert None to Int".to_string()),
        }
    }

    pub fn to_float(&self) -> Result<f64, String> {
        match self {
            Value::Int(value) => Ok(*value as f64),
            Value::Float(value) if value.is_finite() => Ok(*value),
            Value::Float(_) => Err("Cannot convert a non-finite Float".into()),
            Value::String(value) => {
                let number = value
                    .parse::<f64>()
                    .map_err(|_| format!("Cannot convert '{}' to Float", value))?;
                if number.is_finite() {
                    Ok(number)
                } else {
                    Err(format!("Cannot convert '{}' to a finite Float", value))
                }
            }
            Value::Bool(value) => Ok(if *value { 1.0 } else { 0.0 }),
            Value::List(_) => Err("Cannot convert List to Float".to_string()),
            Value::Dictionary(_) => Err("Cannot convert Dictionary to Float".to_string()),
            Value::NativeFunction(_) => Err("Cannot convert Function to Float".to_string()),
            Value::UiWindow(_) | Value::UiComponent(_) => {
                Err("Cannot convert UI handle to Float".to_string())
            }
            Value::None => Err("Cannot convert None to Float".to_string()),
        }
    }

    pub fn to_string_value(&self) -> String {
        match self {
            Value::Int(value) => value.to_string(),
            Value::Float(value) => value.to_string(),
            Value::String(value) => value.clone(),
            Value::Bool(value) => value.to_string(),
            Value::List(_)
            | Value::Dictionary(_)
            | Value::NativeFunction(_)
            | Value::UiWindow(_)
            | Value::UiComponent(_) => self.to_string(),
            Value::None => "none".to_string(),
        }
    }

    pub fn to_bool(&self) -> Result<bool, String> {
        match self {
            Value::Bool(value) => Ok(*value),
            Value::Int(value) => Ok(*value != 0),
            Value::Float(value) if value.is_finite() => Ok(*value != 0.0),
            Value::Float(_) => Err("Cannot convert a non-finite Float to Bool".into()),
            Value::String(value) => Ok(!value.is_empty()),
            Value::List(values) => Ok(!values.is_empty()),
            Value::Dictionary(entries) => Ok(!entries.is_empty()),
            Value::NativeFunction(_) => Ok(true),
            Value::UiWindow(_) | Value::UiComponent(_) => Ok(true),
            Value::None => Ok(false),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(value) => write!(f, "{}", value),
            Value::Float(value) => write!(f, "{}", value),
            Value::String(value) => write!(f, "{}", value),
            Value::Bool(value) => write!(f, "{}", value),
            Value::List(values) => {
                write!(f, "[")?;
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", value)?;
                }
                write!(f, "]")
            }
            Value::Dictionary(entries) => {
                write!(f, "{{")?;
                for (index, (key, value)) in normalize_dictionary(entries).iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", key, value)?;
                }
                write!(f, "}}")
            }
            Value::NativeFunction(function) => {
                write!(f, "<native function {}.{}>", function.module, function.name)
            }
            Value::UiWindow(id) => write!(f, "<UI window {}>", id),
            Value::UiComponent(id) => write!(f, "<UI component {}>", id),
            Value::None => write!(f, "none"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Dictionary(a), Value::Dictionary(b)) => {
                let a = normalize_dictionary(a);
                let b = normalize_dictionary(b);
                a.len() == b.len()
                    && a.iter().all(|(key, value)| {
                        b.iter().any(|(other_key, other_value)| {
                            key == other_key && value == other_value
                        })
                    })
            }
            (Value::NativeFunction(a), Value::NativeFunction(b)) => a == b,
            (Value::UiWindow(a), Value::UiWindow(b)) => a == b,
            (Value::UiComponent(a), Value::UiComponent(b)) => a == b,
            (Value::None, Value::None) => true,
            _ => false,
        }
    }
}

/// Returns dictionary entries with unique keys and last-value-wins semantics.
/// The position of a key is its first insertion position, while its value is
/// taken from its final occurrence.
pub fn normalize_dictionary(entries: &[(Value, Value)]) -> Vec<(Value, Value)> {
    let mut normalized: Vec<(Value, Value)> = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        if let Some((_, existing_value)) = normalized
            .iter_mut()
            .find(|(existing_key, _)| existing_key == key)
        {
            *existing_value = value.clone();
        } else {
            normalized.push((key.clone(), value.clone()));
        }
    }
    normalized
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Int(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(value.to_string())
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl From<Vec<Value>> for Value {
    fn from(value: Vec<Value>) -> Self {
        Value::List(value)
    }
}
