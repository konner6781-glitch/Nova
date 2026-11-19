use nova::stdlib::{builtin_modules, call};
use nova::value::{NativeFunction, Value};

fn native(module: &str, name: &str) -> NativeFunction {
    NativeFunction::new(module, name)
}

fn call_with(module: &str, name: &str, arguments: &[Value]) -> Result<Value, String> {
    call(&native(module, name), arguments)
}

#[test]
fn builtin_registry_exposes_each_implemented_module_and_export() {
    let modules = builtin_modules();
    assert_eq!(modules.keys().count(), 21);
    for module in [
        "Math",
        "Time",
        "String",
        "Collections",
        "UI",
        "Files",
        "Directories",
        "Paths",
        "System",
        "JSON",
        "Random",
        "Crypto",
        "CSV",
        "Logging",
        "DateTime",
        "Process",
        "Program",
        "HTTP",
        "Network",
        "Archive",
        "Image",
    ] {
        assert!(modules.contains_key(module), "missing module {module}");
    }
    for (module, expected) in [
        (
            "Math",
            vec![
                "pi", "e", "tau", "abs", "sqrt", "sin", "cos", "tan", "floor", "ceil", "round",
                "min", "max", "pow",
            ],
        ),
        ("Time", vec!["now", "unix", "sleep", "sleep_ms"]),
        (
            "String",
            vec![
                "upper", "lower", "trim", "contains", "replace", "split", "join",
            ],
        ),
        ("Collections", vec!["len", "contains", "keys", "values"]),
        (
            "Files",
            vec![
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
            ],
        ),
        (
            "Directories",
            vec![
                "create",
                "create_all",
                "delete",
                "delete_all",
                "exists",
                "list",
                "current",
                "change",
                "info",
            ],
        ),
        (
            "Paths",
            vec![
                "join",
                "normalize",
                "absolute",
                "relative",
                "extension",
                "filename",
                "parent",
                "exists",
            ],
        ),
        (
            "System",
            vec![
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
            ],
        ),
        ("JSON", vec!["parse", "stringify", "read", "write"]),
        (
            "Random",
            vec!["integer", "float", "boolean", "choice", "shuffle", "seed"],
        ),
        (
            "Crypto",
            vec!["sha256", "sha512", "hmac_sha256", "random_bytes"],
        ),
        ("CSV", vec!["parse", "stringify", "read", "write"]),
        ("Logging", vec!["info", "warning", "error"]),
        ("DateTime", vec!["now", "today", "parse", "format"]),
        (
            "Process",
            vec!["list", "find", "info", "exists", "start", "is_running"],
        ),
        ("Program", vec!["list", "find", "info", "exists"]),
        ("HTTP", vec!["get", "post", "download"]),
        ("Network", vec!["resolve"]),
        ("Archive", vec!["list", "extract", "create"]),
        ("Image", vec!["load", "width", "height", "dimensions", "resize", "crop", "rotate", "flip", "convert", "pixel"]),
        (
            "UI",
            vec![
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
    ] {
        for name in expected {
            assert!(
                modules[module].contains_key(name),
                "missing {module}.{name}"
            );
        }
    }
}

#[test]
fn math_functions_and_constants_have_stable_numeric_semantics() {
    let math = &builtin_modules()["Math"];
    assert!(
        matches!(math["pi"], Value::Float(value) if (value - std::f64::consts::PI).abs() < f64::EPSILON)
    );
    assert!(
        matches!(math["e"], Value::Float(value) if (value - std::f64::consts::E).abs() < f64::EPSILON)
    );
    assert!(
        matches!(math["tau"], Value::Float(value) if (value - std::f64::consts::TAU).abs() < f64::EPSILON)
    );

    assert_eq!(
        call_with("Math", "abs", &[Value::Int(-8)]).unwrap(),
        Value::Int(8)
    );
    assert!(call_with("Math", "abs", &[Value::Int(i64::MIN)]).is_err());
    assert_eq!(
        call_with("Math", "abs", &[Value::Float(-2.5)]).unwrap(),
        Value::Float(2.5)
    );
    assert_eq!(
        call_with("Math", "sqrt", &[81_i64.into()]).unwrap(),
        Value::Float(9.0)
    );
    assert!(call_with("Math", "sqrt", &[Value::Int(-1)]).is_err());
    assert_eq!(
        call_with("Math", "sin", &[0_i64.into()]).unwrap(),
        Value::Float(0.0)
    );
    assert_eq!(
        call_with("Math", "cos", &[0_i64.into()]).unwrap(),
        Value::Float(1.0)
    );
    assert_eq!(
        call_with("Math", "tan", &[0_i64.into()]).unwrap(),
        Value::Float(0.0)
    );
    assert_eq!(
        call_with("Math", "floor", &[2.9_f64.into()]).unwrap(),
        Value::Int(2)
    );
    assert_eq!(
        call_with("Math", "ceil", &[Value::Float(-2.1)]).unwrap(),
        Value::Int(-2)
    );
    assert_eq!(
        call_with("Math", "round", &[2.5_f64.into()]).unwrap(),
        Value::Int(3)
    );
    assert_eq!(
        call_with("Math", "round", &[Value::Float(-2.5)]).unwrap(),
        Value::Int(-3)
    );
    assert_eq!(
        call_with("Math", "min", &[4_i64.into(), 9_i64.into()]).unwrap(),
        Value::Int(4)
    );
    assert_eq!(
        call_with("Math", "max", &[4_i64.into(), 9.5_f64.into()]).unwrap(),
        Value::Float(9.5)
    );
    assert_eq!(
        call_with("Math", "pow", &[2_i64.into(), 3_i64.into()]).unwrap(),
        Value::Float(8.0)
    );
    assert!(call_with("Math", "pow", &[Value::Int(-1), 0.5_f64.into()]).is_err());
}

#[test]
fn time_functions_return_seconds_and_validate_blocking_delays() {
    let now = call_with("Time", "now", &[]).unwrap();
    assert!(matches!(now, Value::Float(value) if value.is_finite() && value > 0.0));
    let unix = call_with("Time", "unix", &[]).unwrap();
    assert!(matches!(unix, Value::Int(value) if value > 0));
    assert_eq!(
        call_with("Time", "sleep", &[0_i64.into()]).unwrap(),
        Value::None
    );
    assert_eq!(
        call_with("Time", "sleep_ms", &[0_i64.into()]).unwrap(),
        Value::None
    );
    assert!(call_with("Time", "sleep", &[Value::Int(-1)]).is_err());
    assert!(call_with("Time", "sleep_ms", &[Value::String("fast".into())]).is_err());
}

#[test]
fn string_functions_support_unicode_and_empty_inputs() {
    assert_eq!(
        call_with("String", "upper", &["stra\u{00DF}e".into()]).unwrap(),
        Value::from("STRASSE")
    );
    assert_eq!(
        call_with("String", "lower", &["NOVA".into()]).unwrap(),
        Value::from("nova")
    );
    assert_eq!(
        call_with("String", "trim", &["  hi  ".into()]).unwrap(),
        Value::from("hi")
    );
    assert_eq!(
        call_with("String", "contains", &["naïve".into(), "ïv".into()]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        call_with(
            "String",
            "replace",
            &["aaaa".into(), "aa".into(), "b".into()]
        )
        .unwrap(),
        Value::from("bb")
    );
    assert_eq!(
        call_with("String", "split", &["".into(), ",".into()]).unwrap(),
        Value::List(vec![Value::String(String::new())])
    );
    assert!(call_with("String", "split", &["a".into(), "".into()]).is_err());
    assert_eq!(
        call_with(
            "String",
            "join",
            &[
                "-".into(),
                Value::List(vec!["a".into(), 2_i64.into(), Value::None])
            ]
        )
        .unwrap(),
        Value::from("a-2-none")
    );
    assert_eq!(
        call_with("String", "join", &["-".into(), Value::List(vec![])]).unwrap(),
        Value::from("")
    );
}

#[test]
fn collection_functions_use_unicode_scalar_and_dictionary_semantics() {
    assert_eq!(
        call_with("Collections", "len", &["a🦊".into()]).unwrap(),
        Value::Int(2)
    );
    assert_eq!(
        call_with("Collections", "len", &[Value::List(vec![1_i64.into()])]).unwrap(),
        Value::Int(1)
    );
    let duplicate_keys = Value::Dictionary(vec![
        (Value::from("a"), 1_i64.into()),
        (Value::from("b"), 2_i64.into()),
        (Value::from("a"), 3_i64.into()),
    ]);
    assert_eq!(
        call_with("Collections", "len", std::slice::from_ref(&duplicate_keys)).unwrap(),
        Value::Int(2)
    );
    assert_eq!(
        call_with(
            "Collections",
            "contains",
            &[duplicate_keys.clone(), Value::from("a")]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        call_with("Collections", "keys", std::slice::from_ref(&duplicate_keys)).unwrap(),
        Value::List(vec![Value::from("a"), Value::from("b")])
    );
    assert_eq!(
        call_with("Collections", "values", &[duplicate_keys]).unwrap(),
        Value::List(vec![3_i64.into(), 2_i64.into()])
    );
}

#[test]
fn every_registered_standard_library_function_checks_arity() {
    let modules = builtin_modules();
    for (module, exports) in modules {
        for (name, value) in exports {
            if matches!(value, Value::NativeFunction(_)) {
                let arguments = vec![Value::None; 9];
                assert!(
                    call_with(&module, &name, &arguments).is_err(),
                    "{module}.{name} accepted an invalid argument count"
                );
            }
        }
    }
}

#[test]
fn files_directories_paths_and_json_have_real_behavior() {
    let root = std::env::temp_dir().join(format!("nova-stdlib-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("café.json");
    let path = file.to_string_lossy().into_owned();
    assert_eq!(
        call_with(
            "Files",
            "write",
            &[path.clone().into(), "{\"name\":\"Nova 🦊\"}".into()]
        )
        .unwrap(),
        Value::None
    );
    assert_eq!(
        call_with("Files", "read", &[path.clone().into()]).unwrap(),
        Value::from("{\"name\":\"Nova 🦊\"}")
    );
    assert_eq!(
        call_with("Files", "append", &[path.clone().into(), "!".into()]).unwrap(),
        Value::None
    );
    assert_eq!(
        call_with("Files", "read_lines", &[path.clone().into()]).unwrap(),
        Value::List(vec![Value::from("{\"name\":\"Nova 🦊\"}!")])
    );
    assert_eq!(
        call_with("Files", "size", &[path.clone().into()]).unwrap(),
        Value::Int("{\"name\":\"Nova 🦊\"}!".len() as i64)
    );
    assert_eq!(
        call_with("Files", "extension", &[path.clone().into()]).unwrap(),
        Value::from("json")
    );
    assert_eq!(
        call_with(
            "Files",
            "write_bytes",
            &[
                path.clone().into(),
                Value::List(vec![65_i64.into(), 66_i64.into()])
            ]
        )
        .unwrap(),
        Value::None
    );
    assert_eq!(
        call_with("Files", "read_bytes", &[path.clone().into()]).unwrap(),
        Value::List(vec![65_i64.into(), 66_i64.into()])
    );
    let tree = call_with(
        "JSON",
        "parse",
        &["{\"n\":[1,true,null],\"s\":\"🦊\"}".into()],
    )
    .unwrap();
    let encoded = call_with("JSON", "stringify", std::slice::from_ref(&tree)).unwrap();
    assert_eq!(call_with("JSON", "parse", &[encoded]).unwrap(), tree);
    assert!(
        call_with(
            "JSON",
            "stringify",
            &[Value::Dictionary(vec![(1_i64.into(), 2_i64.into())])]
        )
        .is_err()
    );
    assert_eq!(
        call_with(
            "Paths",
            "join",
            &[
                root.to_string_lossy().to_string().into(),
                "café.json".into()
            ]
        )
        .unwrap(),
        Value::from(path)
    );
    call_with(
        "Directories",
        "create_all",
        &[root
            .join("nested/child")
            .to_string_lossy()
            .to_string()
            .into()],
    )
    .unwrap();
    assert_eq!(
        call_with(
            "Directories",
            "exists",
            &[root.join("nested").to_string_lossy().to_string().into()]
        )
        .unwrap(),
        Value::Bool(true)
    );
    call_with(
        "Directories",
        "delete_all",
        &[root.to_string_lossy().to_string().into()],
    )
    .unwrap();
}

#[test]
fn system_functions_return_live_host_information() {
    assert!(matches!(call_with("System", "os", &[]).unwrap(), Value::String(s) if !s.is_empty()));
    assert!(
        matches!(call_with("System", "architecture", &[]).unwrap(), Value::String(s) if !s.is_empty())
    );
    assert_eq!(
        call_with("System", "available_parallelism", &[])
            .unwrap()
            .type_name(),
        "Int"
    );
    let key = format!("NOVA_TEST_{}", std::process::id());
    assert_eq!(
        call_with(
            "System",
            "set_environment",
            &[key.clone().into(), "value".into()]
        )
        .unwrap(),
        Value::None
    );
    assert_eq!(
        call_with("System", "environment", &[key.into()]).unwrap(),
        Value::from("value")
    );
}

#[test]
fn crypto_random_csv_and_logging_behave_consistently() {
    assert_eq!(
        call_with("Crypto", "sha256", &["abc".into()]).unwrap(),
        Value::from("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    );
    assert_eq!(
        call_with("Crypto", "sha512", &["abc".into()])
            .unwrap()
            .to_string_value()
            .len(),
        128
    );
    assert_eq!(
        call_with(
            "Crypto",
            "hmac_sha256",
            &[
                "key".into(),
                "The quick brown fox jumps over the lazy dog".into()
            ]
        )
        .unwrap(),
        Value::from("f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8")
    );
    assert_eq!(
        call_with("Crypto", "random_bytes", &[32_i64.into()])
            .unwrap()
            .type_name(),
        "List"
    );
    assert!(call_with("Crypto", "random_bytes", &[1_048_577_i64.into()]).is_err());
    let parsed = call_with(
        "CSV",
        "parse",
        &["name,quote\r\nNova,\"hello, \"\"world\"\"\"\r\n".into()],
    )
    .unwrap();
    assert_eq!(
        parsed,
        Value::List(vec![
            Value::List(vec!["name".into(), "quote".into()]),
            Value::List(vec!["Nova".into(), "hello, \"world\"".into()])
        ])
    );
    let encoded = call_with("CSV", "stringify", std::slice::from_ref(&parsed)).unwrap();
    assert_eq!(call_with("CSV", "parse", &[encoded]).unwrap(), parsed);
    assert!(call_with("CSV", "parse", &["\"open".into()]).is_err());
    call_with("Random", "seed", &[77_i64.into()]).unwrap();
    let first = call_with("Random", "integer", &[1_i64.into(), 100_i64.into()]).unwrap();
    call_with("Random", "seed", &[77_i64.into()]).unwrap();
    assert_eq!(
        first,
        call_with("Random", "integer", &[1_i64.into(), 100_i64.into()]).unwrap()
    );
    assert!(call_with("Random", "choice", &[Value::List(vec![])]).is_err());
    assert_eq!(
        call_with("Logging", "info", &["test".into()]).unwrap(),
        Value::None
    );
}

#[test]
fn datetime_round_trips_iso_utc_dates_and_validates_calendar_values() {
    let epoch = call_with("DateTime", "parse", &["1970-01-01T00:00:00Z".into()]).unwrap();
    assert_eq!(epoch, Value::Int(0));
    assert_eq!(
        call_with("DateTime", "format", &[epoch, "%Y-%m-%d %H:%M:%S".into()]).unwrap(),
        Value::from("1970-01-01 00:00:00")
    );
    let leap = call_with("DateTime", "parse", &["2000-02-29".into()]).unwrap();
    assert_eq!(
        call_with("DateTime", "format", &[leap, "%Y-%m-%d".into()]).unwrap(),
        Value::from("2000-02-29")
    );
    assert!(call_with("DateTime", "parse", &["1900-02-29".into()]).is_err());
}

#[test]
fn process_and_program_discovery_use_live_process_data() {
    let processes = call_with("Process", "list", &[]).unwrap();
    let Value::List(items) = processes else {
        panic!("expected process list")
    };
    assert!(!items.is_empty());
    let current = std::process::id() as i64;
    let info = call_with("Process", "info", &[current.into()]).unwrap();
    assert!(matches!(info, Value::Dictionary(_)));
    assert_eq!(
        call_with("Process", "exists", &[current.into()]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        call_with("Process", "is_running", &[current.into()]).unwrap(),
        Value::Bool(true)
    );
    assert!(matches!(
        call_with(
            "Program",
            "find",
            &[std::env::current_exe()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string()
                .into()]
        )
        .unwrap(),
        Value::List(_)
    ));
}

#[test]
fn http_uses_bounded_responses_and_handles_local_success_and_post() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf).unwrap();
            let req = String::from_utf8_lossy(&buf);
            let (status, body) = if req.starts_with("POST") {
                ("200 OK", "posted")
            } else if req.starts_with("GET") {
                ("201 Created", "hello")
            } else {
                ("400 Bad Request", "bad")
            };
            write!(stream,"HTTP/1.1 {}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{}",status,body.len(),body).unwrap();
        }
    });
    let url = format!("http://{}/", addr);
    let response = call_with("HTTP", "get", &[url.clone().into()]).unwrap();
    let Value::Dictionary(fields) = response else {
        panic!("expected response dictionary")
    };
    assert!(fields.contains(&(Value::from("status"), Value::Int(201))));
    assert!(fields.contains(&(Value::from("body"), Value::from("hello"))));
    let response = call_with("HTTP", "post", &[url.into(), "payload".into()]).unwrap();
    assert!(response.to_string_value().contains("posted"));
    server.join().unwrap();
    assert!(call_with("HTTP", "get", &["file:///etc/passwd".into()]).is_err());
    assert!(matches!(
        call_with("Network", "resolve", &["localhost".into()]).unwrap(),
        Value::List(_)
    ));
}

#[test]
fn image_processing_and_safe_zip_archives_work(){
    use std::io::Write;
    let root=std::env::temp_dir().join(format!("nova-media-{}",std::process::id()));let _=std::fs::remove_dir_all(&root);std::fs::create_dir_all(&root).unwrap();let src=root.join("source.png");let resized=root.join("resized.png");let cropped=root.join("cropped.png");let zip=root.join("bundle.zip");let extracted=root.join("out");
    image::RgbaImage::from_pixel(4,3,image::Rgba([10,20,30,255])).save(&src).unwrap();let loaded=call_with("Image","load", &[src.to_string_lossy().to_string().into()]).unwrap();assert_eq!(call_with("Image","dimensions",std::slice::from_ref(&loaded)).unwrap(),Value::List(vec![4_i64.into(),3_i64.into()]));assert_eq!(call_with("Image","pixel", &[loaded.clone(),0_i64.into(),0_i64.into()]).unwrap(),Value::List(vec![10_i64.into(),20_i64.into(),30_i64.into(),255_i64.into()]));let resized_val=call_with("Image","resize", &[loaded.clone(),resized.to_string_lossy().to_string().into(),2_i64.into(),2_i64.into()]).unwrap();assert_eq!(call_with("Image","width",&[resized_val]).unwrap(),Value::Int(2));call_with("Image","crop", &[loaded.clone(),cropped.to_string_lossy().to_string().into(),1_i64.into(),1_i64.into(),2_i64.into(),2_i64.into()]).unwrap();assert!(call_with("Image","crop", &[loaded,root.join("bad.png").to_string_lossy().to_string().into(),3_i64.into(),2_i64.into(),2_i64.into(),2_i64.into()]).is_err());
    call_with("Archive","create", &[zip.to_string_lossy().to_string().into(),Value::List(vec![src.to_string_lossy().to_string().into()])]).unwrap();assert_eq!(call_with("Archive","list", &[zip.to_string_lossy().to_string().into()]).unwrap(),Value::List(vec![Value::from("source.png")]));call_with("Archive","extract", &[zip.to_string_lossy().to_string().into(),extracted.to_string_lossy().to_string().into()]).unwrap();assert!(extracted.join("source.png").exists());
    let malicious=root.join("malicious.zip");let f=std::fs::File::create(&malicious).unwrap();let mut writer=zip::ZipWriter::new(f);writer.start_file("../outside.txt",zip::write::SimpleFileOptions::default()).unwrap();writer.write_all(b"no").unwrap();writer.finish().unwrap();let bad_dest=root.join("badout");assert!(call_with("Archive","extract", &[malicious.to_string_lossy().to_string().into(),bad_dest.to_string_lossy().to_string().into()]).is_err());assert!(!root.join("outside.txt").exists());let _=std::fs::remove_dir_all(root);
}
