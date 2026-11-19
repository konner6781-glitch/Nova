use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use nova::compiler;
use nova::lexer::Lexer;
use nova::parser::Parser;
use nova::vm::Vm;

fn run_source(source: &str) -> Result<Vec<String>, String> {
    let tokens = Lexer::new(source).tokenize()?;
    let program = Parser::new(tokens).parse()?;
    let chunk = compiler::compile(&program)?;
    let mut vm = Vm::new(chunk);
    vm.run().map_err(|error| error.to_string())?;
    Ok(vm.take_output())
}

#[test]
fn every_non_ui_example_compiles_and_runs_with_expected_output() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut paths = fs::read_dir(&examples)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension().and_then(|extension| extension.to_str()) == Some("nova")
                && !path
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap()
                    .starts_with("ui_")
        })
        .collect::<Vec<_>>();
    paths.sort();

    let expected = [
        (
            "built_in_libraries",
            "9 5 4 NOVA math | time | string 3 true",
        ),
        ("collections", "[1, 7, 3] {enabled: true}"),
        ("conditions", "Passed"),
        ("hello", "Hello from Nova!"),
        ("variables", "fifteen true"),
    ];
    assert_eq!(
        paths.len(),
        expected.len(),
        "update expected example outputs"
    );
    for path in paths {
        let name = path.file_stem().unwrap().to_str().unwrap();
        let output = run_source(&fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|error| panic!("{} failed: {error}", path.display()));
        let expected_output = expected
            .iter()
            .find(|(example, _)| *example == name)
            .unwrap_or_else(|| panic!("no expected output recorded for {name}"))
            .1;
        assert_eq!(output, vec![expected_output], "{}", path.display());
    }
}

#[test]
fn cli_accepts_a_source_path_containing_spaces() {
    let directory = std::env::temp_dir().join(format!("nova cli smoke {}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let source_path: PathBuf = directory.join("program with spaces.nova");
    fs::write(&source_path, "print(\"space path works\")\nstop\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nova"))
        .arg(&source_path)
        .output()
        .expect("Nova CLI should start");
    let _ = fs::remove_dir_all(&directory);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "space path works\n"
    );
}

#[test]
fn cli_errors_include_the_source_path_and_runtime_location() {
    let directory = std::env::temp_dir().join(format!("nova cli error {}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let source_path: PathBuf = directory.join("bad program.nova");
    fs::write(&source_path, "value = 10 / 0\nstop\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nova"))
        .arg(&source_path)
        .output()
        .expect("Nova CLI should start");
    let _ = fs::remove_dir_all(&directory);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(&source_path.display().to_string()));
    assert!(stderr.contains("line 1, column 12"));
}
