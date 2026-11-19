use std::fs;
use std::path::Path;

use nova::compiler;
use nova::lexer::Lexer;
use nova::parser::Parser;
use nova::value::Value;
use nova::vm::Vm;

fn compile_source(source: &str) -> Result<nova::bytecode::Chunk, String> {
    let tokens = Lexer::new(source).tokenize()?;
    let program = Parser::new(tokens).parse()?;
    compiler::compile(&program)
}

#[test]
fn ui_module_is_imported_and_builds_components_through_the_vm() {
    let chunk = compile_source(
        "get UI\nfunction clicked(status):\n    get UI\n    UI.set_text(status, \"Clicked\")\nwindow = UI.window(\"Test\", 640, 420)\nstatus = UI.text(window, \"Waiting\")\nbutton = UI.button(window, \"Go\", {\"on_click\": \"clicked\", \"on_click_args\": [status]})\nstop",
    )
    .unwrap();
    let mut vm = Vm::new(chunk);
    vm.run().unwrap();
    assert!(matches!(
        vm.globals().get("window"),
        Some(Value::UiWindow(_))
    ));
    assert!(matches!(
        vm.globals().get("status"),
        Some(Value::UiComponent(_))
    ));
    assert!(matches!(
        vm.globals().get("button"),
        Some(Value::UiComponent(_))
    ));
    assert!(vm.globals().contains_key("UI"));
}

#[test]
fn vm_rejects_ui_callbacks_with_the_wrong_signature() {
    let chunk = compile_source(
        "get UI\nfunction clicked():\n    print(\"clicked\")\nwindow = UI.window(\"Test\", 640, 420)\nbutton = UI.button(window, \"Go\", {\"on_click\": \"clicked\", \"on_click_args\": [window]})\nstop",
    )
    .unwrap();
    let mut vm = Vm::new(chunk);
    assert!(
        vm.run()
            .unwrap_err()
            .to_string()
            .contains("binding supplies 1")
    );
}

#[test]
fn all_checked_in_ui_examples_parse_and_compile() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut found = 0;
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("nova")
            || !path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap()
                .starts_with("ui_")
        {
            continue;
        }
        found += 1;
        let source = fs::read_to_string(&path).unwrap();
        compile_source(&source).unwrap_or_else(|error| panic!("{}: {}", path.display(), error));
    }
    assert!(found >= 8, "expected all eight UI examples to be present");
}
