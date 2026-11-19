use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::value::{NativeFunction, Value, normalize_dictionary};

#[path = "stdlib_ext.rs"]
mod stdlib_ext;
#[path = "stdlib_extra.rs"]
mod stdlib_extra;
#[path = "stdlib_media.rs"]
mod stdlib_media;

/// Returns Nova's built-in module values. Source imports keep Nova's `get`
/// spelling; this registry is shared by the command-line runner and embedders.
pub fn builtin_modules() -> HashMap<String, HashMap<String, Value>> {
    let mut modules = HashMap::new();

    let mut math = module_functions(
        "Math",
        &[
            "abs", "sqrt", "sin", "cos", "tan", "floor", "ceil", "round", "min", "max", "pow",
        ],
    );
    math.insert("pi".into(), Value::Float(std::f64::consts::PI));
    math.insert("e".into(), Value::Float(std::f64::consts::E));
    math.insert("tau".into(), Value::Float(std::f64::consts::TAU));
    modules.insert("Math".into(), math);

    modules.insert(
        "Time".into(),
        module_functions("Time", &["now", "unix", "sleep", "sleep_ms"]),
    );
    modules.insert(
        "String".into(),
        module_functions(
            "String",
            &[
                "upper", "lower", "trim", "contains", "replace", "split", "join",
            ],
        ),
    );
    modules.insert(
        "Collections".into(),
        module_functions("Collections", &["len", "contains", "keys", "values"]),
    );
    for (module, names) in [
        (
            "Files",
            &[
                "read",
                "read_bytes",
                "read_lines",
                "write",
                "append",
                "write_bytes",
                "exists",
                "size",
                "extension",
                "name",
                "parent",
                "is_file",
                "info",
                "copy",
                "move",
                "rename",
                "delete",
                "create",
            ][..],
        ),
        (
            "Directories",
            &[
                "create",
                "create_all",
                "delete",
                "delete_all",
                "exists",
                "list",
                "current",
                "change",
                "info",
            ][..],
        ),
        (
            "Paths",
            &[
                "join",
                "normalize",
                "absolute",
                "relative",
                "extension",
                "filename",
                "parent",
                "exists",
            ][..],
        ),
        (
            "System",
            &[
                "os",
                "architecture",
                "username",
                "hostname",
                "current_directory",
                "executable_path",
                "home_directory",
                "temporary_directory",
                "environment",
                "set_environment",
                "available_parallelism",
            ][..],
        ),
        ("JSON", &["parse", "stringify", "read", "write"][..]),
        (
            "Random",
            &["integer", "float", "boolean", "choice", "shuffle", "seed"][..],
        ),
        (
            "Crypto",
            &["sha256", "sha512", "hmac_sha256", "random_bytes"][..],
        ),
        ("CSV", &["parse", "stringify", "read", "write"][..]),
        ("Logging", &["info", "warning", "error"][..]),
        ("DateTime", &["now", "today", "parse", "format"][..]),
        (
            "Process",
            &["list", "find", "info", "exists", "start", "is_running"][..],
        ),
        ("Program", &["list", "find", "info", "exists"][..]),
        ("HTTP", &["get", "post", "download"][..]),
        ("Network", &["resolve"][..]),
        ("Archive", &["list", "extract", "create"][..]),
        ("Image", &["load", "width", "height", "dimensions", "resize", "crop", "rotate", "flip", "convert", "pixel"][..]),
    ] {
        modules.insert(module.into(), module_functions(module, names));
    }
    modules.insert(
        "UI".into(),
        module_functions(
            "UI",
            &[
                "window",
                "text",
                "button",
                "input",
                "dropdown",
                "checkbox",
                "slider",
                "image",
                "column",
                "row",
                "container",
                "show",
                "close",
                "value",
                "set_text",
                "set_label",
                "set_value",
                "set_visible",
                "set_enabled",
                "set_style",
                "set_options",
                "remove",
                "theme",
            ],
        ),
    );

    modules
}

fn module_functions(module: &str, names: &[&str]) -> HashMap<String, Value> {
    names
        .iter()
        .map(|name| {
            (
                (*name).to_string(),
                Value::NativeFunction(NativeFunction::new(module, *name)),
            )
        })
        .collect()
}

/// Calls one function from a built-in module and returns its Nova value.
pub fn call(function: &NativeFunction, arguments: &[Value]) -> Result<Value, String> {
    match function.module.as_str() {
        "Math" => call_math(&function.name, arguments),
        "Time" => call_time(&function.name, arguments),
        "String" => call_string(&function.name, arguments),
        "Collections" => call_collections(&function.name, arguments),
        "Files" | "Directories" | "Paths" | "System" | "JSON" => {
            stdlib_ext::call(&function.module, &function.name, arguments)
        }
        "Random" | "Crypto" | "CSV" | "Logging" | "DateTime" | "Process" | "Program" | "HTTP"
        | "Network" => {
            stdlib_extra::call(&function.module, &function.name, arguments)
        }
        "Archive" | "Image" => stdlib_media::call(&function.module, &function.name, arguments),
        _ => Err(format!(
            "Native module '{}' has no callable implementation.",
            function.module
        )),
    }
}

fn call_math(name: &str, arguments: &[Value]) -> Result<Value, String> {
    match name {
        "abs" => {
            expect_arity("Math.abs", arguments, 1)?;
            match &arguments[0] {
                Value::Int(value) => value
                    .checked_abs()
                    .map(Value::Int)
                    .ok_or_else(|| "Math.abs integer result is out of range.".into()),
                Value::Float(value) if value.is_finite() => Ok(Value::Float(value.abs())),
                value => Err(type_error("Math.abs", value)),
            }
        }
        "sqrt" => {
            expect_arity("Math.sqrt", arguments, 1)?;
            let value = finite_number("Math.sqrt", &arguments[0])?;
            if value < 0.0 {
                return Err("Math.sqrt requires a nonnegative value.".into());
            }
            Ok(Value::Float(value.sqrt()))
        }
        "sin" | "cos" | "tan" => {
            expect_arity(&format!("Math.{}", name), arguments, 1)?;
            let value = finite_number(&format!("Math.{}", name), &arguments[0])?;
            let result = match name {
                "sin" => value.sin(),
                "cos" => value.cos(),
                "tan" => value.tan(),
                _ => unreachable!(),
            };
            checked_float(&format!("Math.{}", name), result)
        }
        "floor" | "ceil" | "round" => {
            expect_arity(&format!("Math.{}", name), arguments, 1)?;
            let value = finite_number(&format!("Math.{}", name), &arguments[0])?;
            let result = match name {
                "floor" => value.floor(),
                "ceil" => value.ceil(),
                "round" => value.round(),
                _ => unreachable!(),
            };
            float_to_int(&format!("Math.{}", name), result).map(Value::Int)
        }
        "min" | "max" => {
            expect_arity(&format!("Math.{}", name), arguments, 2)?;
            let left = finite_number(&format!("Math.{}", name), &arguments[0])?;
            let right = finite_number(&format!("Math.{}", name), &arguments[1])?;
            if let (Value::Int(a), Value::Int(b)) = (&arguments[0], &arguments[1]) {
                return Ok(Value::Int(if name == "min" {
                    (*a).min(*b)
                } else {
                    (*a).max(*b)
                }));
            }
            Ok(Value::Float(if name == "min" {
                left.min(right)
            } else {
                left.max(right)
            }))
        }
        "pow" => {
            expect_arity("Math.pow", arguments, 2)?;
            let base = finite_number("Math.pow", &arguments[0])?;
            let exponent = finite_number("Math.pow", &arguments[1])?;
            checked_float("Math.pow", base.powf(exponent))
        }
        _ => Err(format!("Math has no function '{}'.", name)),
    }
}

fn call_time(name: &str, arguments: &[Value]) -> Result<Value, String> {
    match name {
        "now" | "unix" => {
            expect_arity(&format!("Time.{}", name), arguments, 0)?;
            let elapsed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "System clock is earlier than the Unix epoch.")?;
            if name == "now" {
                Ok(Value::Float(elapsed.as_secs_f64()))
            } else {
                let seconds = i64::try_from(elapsed.as_secs())
                    .map_err(|_| "Unix timestamp is outside the Int range.")?;
                Ok(Value::Int(seconds))
            }
        }
        "sleep" | "sleep_ms" => {
            expect_arity(&format!("Time.{}", name), arguments, 1)?;
            let value = finite_number(&format!("Time.{}", name), &arguments[0])?;
            if value < 0.0 {
                return Err(format!("Time.{} requires a nonnegative duration.", name));
            }
            let seconds = if name == "sleep_ms" {
                value / 1_000.0
            } else {
                value
            };
            let duration = Duration::try_from_secs_f64(seconds)
                .map_err(|_| format!("Time.{} duration is too large.", name))?;
            std::thread::sleep(duration);
            Ok(Value::None)
        }
        _ => Err(format!("Time has no function '{}'.", name)),
    }
}

fn call_string(name: &str, arguments: &[Value]) -> Result<Value, String> {
    match name {
        "upper" | "lower" | "trim" => {
            expect_arity(&format!("String.{}", name), arguments, 1)?;
            let text = expect_string(&format!("String.{}", name), &arguments[0])?;
            let result = match name {
                "upper" => text.to_uppercase(),
                "lower" => text.to_lowercase(),
                "trim" => text.trim().to_string(),
                _ => unreachable!(),
            };
            Ok(Value::String(result))
        }
        "contains" => {
            expect_arity("String.contains", arguments, 2)?;
            let text = expect_string("String.contains", &arguments[0])?;
            let search = expect_string("String.contains", &arguments[1])?;
            Ok(Value::Bool(text.contains(search)))
        }
        "replace" => {
            expect_arity("String.replace", arguments, 3)?;
            let text = expect_string("String.replace", &arguments[0])?;
            let from = expect_string("String.replace", &arguments[1])?;
            let to = expect_string("String.replace", &arguments[2])?;
            Ok(Value::String(text.replace(from, to)))
        }
        "split" => {
            expect_arity("String.split", arguments, 2)?;
            let text = expect_string("String.split", &arguments[0])?;
            let separator = expect_string("String.split", &arguments[1])?;
            if separator.is_empty() {
                return Err("String.split separator cannot be empty.".into());
            }
            Ok(Value::List(
                text.split(separator)
                    .map(|part| Value::String(part.to_string()))
                    .collect(),
            ))
        }
        "join" => {
            expect_arity("String.join", arguments, 2)?;
            let separator = expect_string("String.join", &arguments[0])?;
            let Value::List(parts) = &arguments[1] else {
                return Err(format!(
                    "String.join expects a List as its second argument, received {}.",
                    arguments[1].type_name()
                ));
            };
            Ok(Value::String(
                parts
                    .iter()
                    .map(Value::to_string_value)
                    .collect::<Vec<_>>()
                    .join(separator),
            ))
        }
        _ => Err(format!("String has no function '{}'.", name)),
    }
}

fn call_collections(name: &str, arguments: &[Value]) -> Result<Value, String> {
    match name {
        "len" => {
            expect_arity("Collections.len", arguments, 1)?;
            let length = match &arguments[0] {
                Value::String(value) => value.chars().count(),
                Value::List(values) => values.len(),
                Value::Dictionary(entries) => normalize_dictionary(entries).len(),
                value => {
                    return Err(format!(
                        "Collections.len does not support {} values.",
                        value.type_name()
                    ));
                }
            };
            let length = i64::try_from(length)
                .map_err(|_| "Collections.len result is outside the Int range.")?;
            Ok(Value::Int(length))
        }
        "contains" => {
            expect_arity("Collections.contains", arguments, 2)?;
            let found = match &arguments[0] {
                Value::List(values) => values.iter().any(|value| value == &arguments[1]),
                Value::Dictionary(entries) => normalize_dictionary(entries)
                    .iter()
                    .any(|(key, _)| key == &arguments[1]),
                value => {
                    return Err(format!(
                        "Collections.contains does not support {} values.",
                        value.type_name()
                    ));
                }
            };
            Ok(Value::Bool(found))
        }
        "keys" | "values" => {
            expect_arity(&format!("Collections.{}", name), arguments, 1)?;
            let Value::Dictionary(entries) = &arguments[0] else {
                return Err(format!(
                    "Collections.{} expects a Dictionary, received {}.",
                    name,
                    arguments[0].type_name()
                ));
            };
            let values = normalize_dictionary(entries)
                .iter()
                .map(|(key, value)| {
                    if name == "keys" {
                        key.clone()
                    } else {
                        value.clone()
                    }
                })
                .collect();
            Ok(Value::List(values))
        }
        _ => Err(format!("Collections has no function '{}'.", name)),
    }
}

fn expect_arity(name: &str, arguments: &[Value], expected: usize) -> Result<(), String> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err(format!(
            "{} expects {} argument{} but received {}.",
            name,
            expected,
            if expected == 1 { "" } else { "s" },
            arguments.len()
        ))
    }
}

fn expect_string<'a>(name: &str, value: &'a Value) -> Result<&'a str, String> {
    match value {
        Value::String(value) => Ok(value),
        _ => Err(format!(
            "{} expects String values, received {}.",
            name,
            value.type_name()
        )),
    }
}

fn finite_number(name: &str, value: &Value) -> Result<f64, String> {
    let number = match value {
        Value::Int(value) => *value as f64,
        Value::Float(value) => *value,
        _ => return Err(type_error(name, value)),
    };
    if number.is_finite() {
        Ok(number)
    } else {
        Err(format!("{} requires a finite number.", name))
    }
}

fn float_to_int(name: &str, value: f64) -> Result<i64, String> {
    if value.is_finite() && value >= i64::MIN as f64 && value < 9_223_372_036_854_775_808.0 {
        Ok(value as i64)
    } else {
        Err(format!("{} result is outside the Int range.", name))
    }
}

fn checked_float(name: &str, value: f64) -> Result<Value, String> {
    if value.is_finite() {
        Ok(Value::Float(value))
    } else {
        Err(format!("{} produced a non-finite result.", name))
    }
}

fn type_error(name: &str, value: &Value) -> String {
    format!(
        "{} expects a numeric value, received {}.",
        name,
        value.type_name()
    )
}
