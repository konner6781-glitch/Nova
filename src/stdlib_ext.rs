use std::fs;
use std::path::{Path, PathBuf};

use crate::value::{Value, normalize_dictionary};

pub fn call(module: &str, name: &str, args: &[Value]) -> Result<Value, String> {
    match module {
        "Files" => files(name, args),
        "Directories" => directories(name, args),
        "Paths" => paths(name, args),
        "System" => system(name, args),
        "JSON" => json(name, args),
        _ => Err(format!("Unknown standard library module '{}'.", module)),
    }
}

fn arity(label: &str, args: &[Value], count: usize) -> Result<(), String> {
    if args.len() == count {
        Ok(())
    } else {
        Err(format!(
            "{} expects {} argument(s), received {}.",
            label,
            count,
            args.len()
        ))
    }
}
fn string<'a>(label: &str, value: &'a Value) -> Result<&'a str, String> {
    if let Value::String(s) = value {
        Ok(s)
    } else {
        Err(format!(
            "{} expects String, received {}.",
            label,
            value.type_name()
        ))
    }
}
fn path_arg<'a>(
    label: &str,
    args: &'a [Value],
    count: usize,
    index: usize,
) -> Result<&'a Path, String> {
    arity(label, args, count)?;
    Ok(Path::new(string(label, &args[index])?))
}
fn io_error(action: &str, path: &Path, error: std::io::Error) -> String {
    format!("{} '{}': {}", action, path.display(), error)
}
fn metadata(path: &Path) -> Result<fs::Metadata, String> {
    fs::metadata(path).map_err(|e| io_error("Cannot inspect", path, e))
}

fn files(name: &str, a: &[Value]) -> Result<Value, String> {
    match name {
        "read" => {
            let p = path_arg("Files.read", a, 1, 0)?;
            fs::read_to_string(p)
                .map(Value::String)
                .map_err(|e| io_error("Cannot read text file", p, e))
        }
        "read_bytes" => {
            let p = path_arg("Files.read_bytes", a, 1, 0)?;
            fs::read(p)
                .map(|b| Value::List(b.into_iter().map(|x| Value::Int(x as i64)).collect()))
                .map_err(|e| io_error("Cannot read file", p, e))
        }
        "read_lines" => {
            let p = path_arg("Files.read_lines", a, 1, 0)?;
            fs::read_to_string(p)
                .map(|s| Value::List(s.lines().map(|l| Value::String(l.into())).collect()))
                .map_err(|e| io_error("Cannot read text file", p, e))
        }
        "write" | "append" => {
            arity(&format!("Files.{}", name), a, 2)?;
            let p = Path::new(string("Files", &a[0])?);
            let data = string("Files", &a[1])?;
            let result = if name == "write" {
                fs::write(p, data)
            } else {
                use std::io::Write;
                fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(p)
                    .and_then(|mut f| f.write_all(data.as_bytes()))
            };
            result.map_err(|e| io_error("Cannot write file", p, e))?;
            Ok(Value::None)
        }
        "write_bytes" => {
            arity("Files.write_bytes", a, 2)?;
            let p = Path::new(string("Files", &a[0])?);
            let Value::List(bytes) = &a[1] else {
                return Err("Files.write_bytes expects a List of integers.".into());
            };
            let bytes =
                bytes
                    .iter()
                    .map(|v| match v {
                        Value::Int(n) if (0..=255).contains(n) => Ok(*n as u8),
                        _ => Err("Files.write_bytes requires byte values from 0 through 255."
                            .to_string()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
            fs::write(p, bytes).map_err(|e| io_error("Cannot write file", p, e))?;
            Ok(Value::None)
        }
        "exists" | "is_file" => {
            let p = path_arg(&format!("Files.{}", name), a, 1, 0)?;
            Ok(Value::Bool(if name == "exists" {
                p.exists()
            } else {
                p.is_file()
            }))
        }
        "size" => {
            let p = path_arg("Files.size", a, 1, 0)?;
            let n = metadata(p)?.len();
            Ok(Value::Int(
                i64::try_from(n).map_err(|_| "File is too large for Nova Int.")?,
            ))
        }
        "extension" | "name" | "parent" => {
            let p = path_arg(&format!("Files.{}", name), a, 1, 0)?;
            let s = match name {
                "extension" => p.extension().and_then(|x| x.to_str()),
                "name" => p.file_name().and_then(|x| x.to_str()),
                _ => p.parent().and_then(Path::to_str),
            };
            Ok(s.map(|x| Value::String(x.into())).unwrap_or(Value::None))
        }
        "info" => {
            let p = path_arg("Files.info", a, 1, 0)?;
            if !p.exists() {
                return Ok(Value::Dictionary(vec![(
                    "exists".into(),
                    Value::Bool(false),
                )]));
            }
            let m = metadata(p)?;
            let mut e = vec![
                ("exists".into(), Value::Bool(true)),
                (
                    "name".into(),
                    p.file_name()
                        .and_then(|x| x.to_str())
                        .map(|x| Value::String(x.into()))
                        .unwrap_or(Value::None),
                ),
                (
                    "size".into(),
                    Value::Int(i64::try_from(m.len()).unwrap_or(i64::MAX)),
                ),
                ("is_file".into(), Value::Bool(m.is_file())),
                ("is_directory".into(), Value::Bool(m.is_dir())),
                (
                    "extension".into(),
                    p.extension()
                        .and_then(|x| x.to_str())
                        .map(|x| Value::String(x.into()))
                        .unwrap_or(Value::None),
                ),
            ];
            if let Ok(t) = m.modified().and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map_err(std::io::Error::other)
            }) {
                e.push((
                    "modified_unix".into(),
                    Value::Int(t.as_secs().min(i64::MAX as u64) as i64),
                ))
            }
            Ok(Value::Dictionary(normalize_dictionary(&e)))
        }
        "copy" | "move" => {
            arity(&format!("Files.{}", name), a, 2)?;
            let src = Path::new(string("Files", &a[0])?);
            let dst = Path::new(string("Files", &a[1])?);
            if dst.exists() {
                return Err(format!("Destination '{}' already exists.", dst.display()));
            }
            let r = if name == "copy" {
                fs::copy(src, dst).map(|_| ())
            } else {
                fs::rename(src, dst)
            };
            r.map_err(|e| io_error("File operation failed", src, e))?;
            Ok(Value::None)
        }
        "rename" => {
            arity("Files.rename", a, 2)?;
            let p = Path::new(string("Files", &a[0])?);
            let new = string("Files", &a[1])?;
            if Path::new(new).components().count() != 1 {
                return Err("Files.rename expects a filename, not a path.".into());
            }
            let dst = p.parent().unwrap_or(Path::new(".")).join(new);
            if dst.exists() {
                return Err(format!("Destination '{}' already exists.", dst.display()));
            }
            fs::rename(p, &dst).map_err(|e| io_error("Cannot rename", p, e))?;
            Ok(Value::None)
        }
        "delete" => {
            let p = path_arg("Files.delete", a, 1, 0)?;
            fs::remove_file(p).map_err(|e| io_error("Cannot delete file", p, e))?;
            Ok(Value::None)
        }
        "create" => {
            let p = path_arg("Files.create", a, 1, 0)?;
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(p)
                .map_err(|e| io_error("Cannot create file", p, e))?;
            Ok(Value::None)
        }
        _ => Err(format!("Files has no function '{}'.", name)),
    }
}

fn directories(name: &str, a: &[Value]) -> Result<Value, String> {
    match name {
        "current" => {
            arity("Directories.current", a, 0)?;
            Ok(Value::String(
                std::env::current_dir()
                    .map_err(|e| format!("Cannot get current directory: {}", e))?
                    .to_string_lossy()
                    .into_owned(),
            ))
        }
        "change" => {
            let p = path_arg("Directories.change", a, 1, 0)?;
            std::env::set_current_dir(p).map_err(|e| io_error("Cannot change directory", p, e))?;
            Ok(Value::None)
        }
        "create" | "create_all" => {
            let p = path_arg(&format!("Directories.{}", name), a, 1, 0)?;
            if name == "create" {
                fs::create_dir(p)
            } else {
                fs::create_dir_all(p)
            }
            .map_err(|e| io_error("Cannot create directory", p, e))?;
            Ok(Value::None)
        }
        "delete" | "delete_all" => {
            let p = path_arg(&format!("Directories.{}", name), a, 1, 0)?;
            let r = if name == "delete" {
                fs::remove_dir(p)
            } else {
                fs::remove_dir_all(p)
            };
            r.map_err(|e| io_error("Cannot delete directory", p, e))?;
            Ok(Value::None)
        }
        "exists" => {
            let p = path_arg("Directories.exists", a, 1, 0)?;
            Ok(Value::Bool(p.is_dir()))
        }
        "list" => {
            let p = path_arg("Directories.list", a, 1, 0)?;
            let iter = fs::read_dir(p).map_err(|e| io_error("Cannot list directory", p, e))?;
            let mut entries = iter
                .map(|x| {
                    x.map(|e| Value::String(e.file_name().to_string_lossy().into_owned()))
                        .map_err(|e| format!("Cannot read directory entry: {}", e))
                })
                .collect::<Result<Vec<_>, _>>()?;
            entries.sort_by_key(Value::to_string_value);
            Ok(Value::List(entries))
        }
        "info" => {
            let p = path_arg("Directories.info", a, 1, 0)?;
            let m = metadata(p)?;
            Ok(Value::Dictionary(vec![
                (
                    "path".into(),
                    Value::String(p.to_string_lossy().into_owned()),
                ),
                ("is_directory".into(), Value::Bool(m.is_dir())),
                (
                    "modified_unix".into(),
                    m.modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|t| Value::Int(t.as_secs().min(i64::MAX as u64) as i64))
                        .unwrap_or(Value::None),
                ),
            ]))
        }
        _ => Err(format!("Directories has no function '{}'.", name)),
    }
}

fn paths(name: &str, a: &[Value]) -> Result<Value, String> {
    let label = format!("Paths.{}", name);
    match name {
        "join" => {
            if a.len() < 2 {
                return Err("Paths.join expects at least 2 arguments.".into());
            }
            let mut p = PathBuf::new();
            for x in a {
                p.push(string(&label, x)?)
            }
            Ok(Value::String(p.to_string_lossy().into_owned()))
        }
        "normalize" | "absolute" | "relative" => {
            let p = path_arg(&label, a, 1, 0)?;
            let result = if name == "absolute" {
                if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    std::env::current_dir()
                        .map_err(|e| format!("Cannot get current directory: {}", e))?
                        .join(p)
                }
            } else if name == "relative" {
                let abs = if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    std::env::current_dir()
                        .map_err(|e| format!("Cannot get current directory: {}", e))?
                        .join(p)
                };
                let cwd = std::env::current_dir()
                    .map_err(|e| format!("Cannot get current directory: {}", e))?;
                abs.strip_prefix(cwd).map(Path::to_path_buf).unwrap_or(abs)
            } else {
                normalize(p)
            };
            Ok(Value::String(result.to_string_lossy().into_owned()))
        }
        "extension" | "filename" | "parent" => {
            let p = path_arg(&label, a, 1, 0)?;
            let s = match name {
                "extension" => p.extension().and_then(|x| x.to_str()),
                "filename" => p.file_name().and_then(|x| x.to_str()),
                _ => p.parent().and_then(Path::to_str),
            };
            Ok(s.map(|s| Value::String(s.into())).unwrap_or(Value::None))
        }
        "exists" => {
            let p = path_arg(&label, a, 1, 0)?;
            Ok(Value::Bool(p.exists()))
        }
        _ => Err(format!("Paths has no function '{}'.", name)),
    }
}
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            _ => out.push(c.as_os_str()),
        }
    }
    out
}

fn system(name: &str, a: &[Value]) -> Result<Value, String> {
    let label = format!("System.{}", name);
    match name {
        "os" => {
            arity(&label, a, 0)?;
            Ok(Value::String(std::env::consts::OS.into()))
        }
        "architecture" => {
            arity(&label, a, 0)?;
            Ok(Value::String(std::env::consts::ARCH.into()))
        }
        "username" => {
            arity(&label, a, 0)?;
            let v = std::env::var("USERNAME")
                .or_else(|_| std::env::var("USER"))
                .map_err(|_| "Username is unavailable.")?;
            Ok(Value::String(v))
        }
        "hostname" => {
            arity(&label, a, 0)?;
            let v = std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .map_err(|_| "Hostname is unavailable.")?;
            Ok(Value::String(v))
        }
        "current_directory" | "executable_path" | "home_directory" | "temporary_directory" => {
            arity(&label, a, 0)?;
            let p = match name {
                "current_directory" => std::env::current_dir().map_err(|e| e.to_string())?,
                "executable_path" => std::env::current_exe().map_err(|e| e.to_string())?,
                "home_directory" => PathBuf::from(
                    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                        .ok_or("Home directory is unavailable.")?,
                ),
                _ => std::env::temp_dir(),
            };
            Ok(Value::String(p.to_string_lossy().into_owned()))
        }
        "environment" => {
            arity(&label, a, 1)?;
            let k = string(&label, &a[0])?;
            Ok(std::env::var(k)
                .ok()
                .map(Value::String)
                .unwrap_or(Value::None))
        }
        "set_environment" => {
            arity(&label, a, 2)?;
            let k = string(&label, &a[0])?;
            let v = string(&label, &a[1])?;
            if k.is_empty() || k.contains('=') || k.contains('\0') {
                return Err("System.set_environment received an invalid variable name.".into());
            }
            unsafe { std::env::set_var(k, v) };
            Ok(Value::None)
        }
        "available_parallelism" => {
            arity(&label, a, 0)?;
            Ok(Value::Int(
                std::thread::available_parallelism()
                    .map(|x| x.get() as i64)
                    .unwrap_or(1),
            ))
        }
        _ => Err(format!("System has no function '{}'.", name)),
    }
}

fn json(name: &str, a: &[Value]) -> Result<Value, String> {
    match name {
        "parse" | "read" => {
            let text = if name == "parse" {
                arity("JSON.parse", a, 1)?;
                string("JSON.parse", &a[0])?.to_owned()
            } else {
                let p = path_arg("JSON.read", a, 1, 0)?;
                fs::read_to_string(p).map_err(|e| io_error("Cannot read JSON", p, e))?
            };
            let v: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| format!("Invalid JSON: {}", e))?;
            from_json(v)
        }
        "stringify" | "write" => {
            let (value, dest) = if name == "stringify" {
                arity("JSON.stringify", a, 1)?;
                (&a[0], None)
            } else {
                arity("JSON.write", a, 2)?;
                (&a[1], Some(Path::new(string("JSON.write", &a[0])?)))
            };
            let v = to_json(value)?;
            let text =
                serde_json::to_string(&v).map_err(|e| format!("Cannot encode JSON: {}", e))?;
            if let Some(p) = dest {
                fs::write(p, text).map_err(|e| io_error("Cannot write JSON", p, e))?;
                Ok(Value::None)
            } else {
                Ok(Value::String(text))
            }
        }
        _ => Err(format!("JSON has no function '{}'.", name)),
    }
}
fn from_json(v: serde_json::Value) -> Result<Value, String> {
    Ok(match v {
        serde_json::Value::Null => Value::None,
        serde_json::Value::Bool(x) => Value::Bool(x),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else {
                Value::Float(
                    n.as_f64()
                        .ok_or("JSON number is outside Nova's numeric range.")?,
                )
            }
        }
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(a) => {
            Value::List(a.into_iter().map(from_json).collect::<Result<_, _>>()?)
        }
        serde_json::Value::Object(o) => Value::Dictionary(
            o.into_iter()
                .map(|(k, v)| Ok((Value::String(k), from_json(v)?)))
                .collect::<Result<_, String>>()?,
        ),
    })
}
fn to_json(v: &Value) -> Result<serde_json::Value, String> {
    Ok(match v {
        Value::None => serde_json::Value::Null,
        Value::Bool(x) => (*x).into(),
        Value::Int(x) => (*x).into(),
        Value::Float(x) => serde_json::Number::from_f64(*x)
            .map(Into::into)
            .ok_or("JSON cannot encode a non-finite Float.")?,
        Value::String(s) => s.clone().into(),
        Value::List(xs) => {
            serde_json::Value::Array(xs.iter().map(to_json).collect::<Result<_, _>>()?)
        }
        Value::Dictionary(xs) => {
            let mut o = serde_json::Map::new();
            for (k, v) in normalize_dictionary(xs) {
                let Value::String(k) = k else {
                    return Err("JSON objects require String dictionary keys.".into());
                };
                o.insert(k, to_json(&v)?);
            }
            serde_json::Value::Object(o)
        }
        _ => return Err(format!("JSON cannot encode {} values.", v.type_name())),
    })
}
