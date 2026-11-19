use std::collections::HashMap;

use nova::bytecode::{Chunk, Instruction};
use nova::compiler;
use nova::lexer::Lexer;
use nova::parser::Parser;
use nova::value::Value;
use nova::vm::Vm;

fn compile_source(source: &str) -> Result<nova::bytecode::Chunk, String> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    let program = parser.parse()?;
    compiler::compile(&program)
}

fn run_source(source: &str) -> Vm {
    let chunk = compile_source(source).expect("source should compile");
    let mut vm = Vm::new(chunk);
    vm.run().expect("bytecode should run");
    vm
}

#[test]
fn executes_dynamic_variables_arithmetic_and_print() {
    let vm = run_source("age = 15\nage = \"fifteen\"\nscore = 10 + 5 * 2\nprint(age, score)\nstop");
    assert_eq!(vm.globals().get("age"), Some(&Value::from("fifteen")));
    assert_eq!(vm.globals().get("score"), Some(&Value::Int(20)));
    assert_eq!(vm.output(), &["fifteen 20".to_string()]);
}

#[test]
fn source_conversion_functions_execute_and_validate_inputs() {
    let vm = run_source(
        "whole = int(\"12\")\nratio = float(3)\nrendered = str(true)\nempty = bool([])\nprint(whole, ratio, rendered, empty)\nstop",
    );
    assert_eq!(vm.globals().get("whole"), Some(&Value::Int(12)));
    assert_eq!(vm.globals().get("ratio"), Some(&Value::Float(3.0)));
    assert_eq!(vm.globals().get("rendered"), Some(&Value::from("true")));
    assert_eq!(vm.globals().get("empty"), Some(&Value::Bool(false)));
    assert_eq!(vm.output(), &["12 3 true false".to_string()]);

    let chunk = compile_source("value = int(\"not a number\")\nstop").unwrap();
    assert!(
        Vm::new(chunk)
            .run()
            .unwrap_err()
            .to_string()
            .contains("Cannot convert")
    );

    let chunk = compile_source("value = bool()\nstop").unwrap();
    assert!(
        Vm::new(chunk)
            .run()
            .unwrap_err()
            .to_string()
            .contains("expects 1 argument")
    );
}

#[test]
fn string_escapes_survive_compilation_and_decode_once() {
    let source = r#"value = "n:\n t:\t r:\r slash:\\ quote:\" apostrophe:\'"
print(value)
stop"#;
    let vm = run_source(source);
    assert_eq!(
        vm.output(),
        &["n:\n t:\t r:\r slash:\\ quote:\" apostrophe:'".to_string()]
    );
}

#[test]
fn float_conversion_rejects_non_finite_values() {
    let error = compile_source("value = float(\"NaN\")\nstop")
        .and_then(|chunk| Vm::new(chunk).run().map_err(|error| error.to_string()))
        .unwrap_err();
    assert!(error.contains("finite Float"));
}

#[test]
fn vm_rejects_non_finite_values_from_host_modules_and_bytecode() {
    let chunk = compile_source("get External\nstop").unwrap();
    let mut vm = Vm::new(chunk);
    vm.register_module(
        "External",
        HashMap::from([(
            "nested".into(),
            Value::List(vec![Value::Float(f64::INFINITY)]),
        )]),
    );
    assert!(
        vm.run()
            .unwrap_err()
            .to_string()
            .contains("Non-finite Float values")
    );

    let mut chunk = Chunk::new();
    let constant = chunk.add_constant(Value::Float(f64::NAN));
    chunk.write(Instruction::LoadConstant(constant));
    chunk.write(Instruction::Stop);
    assert!(
        Vm::new(chunk)
            .run()
            .unwrap_err()
            .to_string()
            .contains("Non-finite Float values")
    );
}

#[test]
fn runtime_errors_report_the_operator_line_and_column() {
    let chunk = compile_source("answer = 10 / 0\nstop").unwrap();
    let mut vm = Vm::new(chunk);
    let error = vm.run().unwrap_err();
    assert_eq!(
        error.location(),
        Some(nova::tokens::SourceLocation {
            line: 1,
            column: 13
        })
    );
    assert!(error.to_string().contains("line 1, column 13"));
}

#[test]
fn executes_if_elif_else() {
    let vm = run_source(
        "score = 75\nif score > 90:\n    grade = \"A\"\nelif score >= 70:\n    grade = \"B\"\nelse:\n    grade = \"C\"\nstop",
    );
    assert_eq!(vm.globals().get("grade"), Some(&Value::from("B")));
}

#[test]
fn counted_conditional_limited_and_delayed_loops_work() {
    let vm = run_source(
        "counted = 0\nloop.repeat 3, delay(0):\n    counted += 1\nconditional = 0\nloop.repeat conditional < 10, 5:\n    conditional += 1\ncombined = 0\nloop.repeat combined < 10; 3, delay(0):\n    combined += 1\nstop",
    );
    assert_eq!(vm.globals().get("counted"), Some(&Value::Int(3)));
    assert_eq!(vm.globals().get("conditional"), Some(&Value::Int(5)));
    assert_eq!(vm.globals().get("combined"), Some(&Value::Int(3)));
}

#[test]
fn zero_iteration_limit_skips_condition_evaluation() {
    let vm = run_source("loop.repeat missing_value > 0, 0:\n    print(\"unreachable\")\nstop");
    assert!(vm.output().is_empty());
}

#[test]
fn nested_break_exits_only_the_innermost_loop() {
    let vm = run_source(
        "outer = 0\ninner = 0\nloop.repeat 3:\n    outer += 1\n    loop:\n        inner += 1\n        break\n    inner += 10\nstop",
    );
    assert_eq!(vm.globals().get("outer"), Some(&Value::Int(3)));
    assert_eq!(vm.globals().get("inner"), Some(&Value::Int(33)));
}

#[test]
fn infinite_loop_can_exit_with_break() {
    let vm = run_source("count = 0\nloop:\n    count += 1\n    break\nstop");
    assert_eq!(vm.globals().get("count"), Some(&Value::Int(1)));
}

#[test]
fn functions_support_parameters_recursion_and_local_scope() {
    let vm = run_source(
        "global_value = 7\nfunction factorial(n):\n    local_value = n\n    if n <= 1:\n        return(1)\n    return(n * factorial(n - 1))\nfunction local_test():\n    global_value = 99\n    return(global_value)\nanswer = factorial(5)\nlocal_result = local_test()\nprint(answer, local_result, global_value)\nstop",
    );
    assert_eq!(vm.globals().get("answer"), Some(&Value::Int(120)));
    assert_eq!(vm.globals().get("local_result"), Some(&Value::Int(99)));
    assert_eq!(vm.globals().get("global_value"), Some(&Value::Int(7)));
    assert_eq!(vm.output(), &["120 99 7".to_string()]);
}

#[test]
fn functions_do_not_capture_global_variables() {
    let chunk = compile_source(
        "outside = 4\nfunction read_outside():\n    return(outside)\nresult = read_outside()\nstop",
    )
    .unwrap();
    let mut vm = Vm::new(chunk);
    let error = vm.run().unwrap_err();
    assert!(error.to_string().contains("Undefined variable 'outside'"));
}

#[test]
fn multiple_returns_can_be_kept_or_unpacked() {
    let vm = run_source(
        "function pair():\n    return(10, 20)\nfirst, second = pair()\nall_values = pair()\nprint(first, second, all_values)\nstop",
    );
    assert_eq!(vm.globals().get("first"), Some(&Value::Int(10)));
    assert_eq!(vm.globals().get("second"), Some(&Value::Int(20)));
    assert_eq!(
        vm.globals().get("all_values"),
        Some(&Value::List(vec![Value::Int(10), Value::Int(20)]))
    );
    assert_eq!(vm.output(), &["10 20 [10, 20]".to_string()]);
}

#[test]
fn list_and_dictionary_literals_execute() {
    let vm = run_source(
        "items = [1, \"two\", true]\nsettings = {\"volume\": 5, \"active\": true}\nprint(items, settings)\nstop",
    );
    assert_eq!(
        vm.globals().get("items"),
        Some(&Value::List(vec![
            Value::Int(1),
            Value::from("two"),
            Value::Bool(true),
        ]))
    );
    assert_eq!(
        vm.globals().get("settings"),
        Some(&Value::Dictionary(vec![
            (Value::from("volume"), Value::Int(5)),
            (Value::from("active"), Value::Bool(true)),
        ]))
    );
}

#[test]
fn collections_support_nested_indexing_and_mutation() {
    let vm = run_source(
        "items = [[1, 2], [3, 4]]\nitems[0][1] += 5\nsettings = {\"active\": false}\nsettings[\"active\"] = items[0][1] > 3\nsettings[\"count\"] = 2\nprint(items[0][1], settings[\"active\"], settings[\"count\"])\nstop",
    );

    assert_eq!(
        vm.globals().get("items"),
        Some(&Value::List(vec![
            Value::List(vec![Value::Int(1), Value::Int(7)]),
            Value::List(vec![Value::Int(3), Value::Int(4)]),
        ]))
    );
    assert_eq!(
        vm.globals().get("settings"),
        Some(&Value::Dictionary(vec![
            (Value::from("active"), Value::Bool(true)),
            (Value::from("count"), Value::Int(2)),
        ]))
    );
    assert_eq!(vm.output(), &["7 true 2".to_string()]);
}

#[test]
fn invalid_collection_indexes_report_runtime_errors() {
    let chunk = compile_source("items = [1]\nprint(items[2])\nstop").unwrap();
    assert!(
        Vm::new(chunk)
            .run()
            .unwrap_err()
            .to_string()
            .contains("out of range")
    );

    let chunk = compile_source("settings = {}\nprint(settings[\"missing\"])\nstop").unwrap();
    assert!(
        Vm::new(chunk)
            .run()
            .unwrap_err()
            .to_string()
            .contains("no key")
    );
}

#[test]
fn logical_operators_short_circuit() {
    let vm = run_source("result = false and missing\nother = true or missing\nstop");
    assert_eq!(vm.globals().get("result"), Some(&Value::Bool(false)));
    assert_eq!(vm.globals().get("other"), Some(&Value::Bool(true)));
}

#[test]
fn compile_errors_reject_return_or_break_outside_their_context() {
    assert!(compile_source("return(1)\nstop").is_err());
    assert!(compile_source("break\nstop").is_err());
}

#[test]
fn runtime_errors_cover_bad_operations_and_function_arity() {
    let chunk = compile_source("value = 1 / 0\nstop").unwrap();
    assert!(
        Vm::new(chunk)
            .run()
            .unwrap_err()
            .to_string()
            .contains("Division by zero")
    );

    let chunk =
        compile_source("function add(a, b):\n    return(a + b)\nvalue = add(1)\nstop").unwrap();
    assert!(
        Vm::new(chunk)
            .run()
            .unwrap_err()
            .to_string()
            .contains("expects 2 arguments")
    );
}

#[test]
fn imports_use_registered_module_values() {
    let chunk = compile_source("get Math\nfrom Math get pi\nprint(pi, Math)\nstop").unwrap();
    let mut vm = Vm::new(chunk);
    let mut exports = HashMap::new();
    exports.insert("pi".to_string(), Value::Float(2.5));
    vm.register_module("Math", exports);
    vm.run().unwrap();

    assert_eq!(vm.globals().get("pi"), Some(&Value::Float(2.5)));
    assert_eq!(vm.output(), &["2.5 {pi: 2.5}".to_string()]);
}

#[test]
fn built_in_math_string_and_collection_modules_work_with_get() {
    let vm = run_source(
        "get Math\nget String\nget Collections\nfrom Math get sqrt\nroot = Math.sqrt(81)\nother_root = sqrt(25)\nrounded = Math.round(3.6)\nabsolute = Math.abs(-5)\nsine = Math.sin(0)\ncosine = Math.cos(0)\ntangent = Math.tan(0)\nfloored = Math.floor(2.9)\nceiled = Math.ceil(2.1)\nminimum = Math.min(3, 2)\nmaximum = Math.max(3, 2)\npower = Math.pow(2, 4)\nuppercase = String.upper(\"nova\")\nlowercase = String.lower(\"NOVA\")\ntrimmed = String.trim(\"  nova  \")\nstring_has = String.contains(\"nova\", \"ov\")\nreplaced = String.replace(\"nova\", \"ov\", \"ew\")\nparts = String.split(\"a,b,c\", \",\")\njoined = String.join(\"-\", parts)\nlength = Collections.len(parts)\ncontains = Collections.contains(parts, \"b\")\nsettings = {\"volume\": 5}\ndictionary_has = Collections.contains(settings, \"volume\")\nkeys = Collections.keys(settings)\nvalues = Collections.values(settings)\nprint(root, other_root, rounded, uppercase, joined, length, contains, keys, Math.pi)\nstop",
    );

    assert_eq!(vm.globals().get("root"), Some(&Value::Float(9.0)));
    assert_eq!(vm.globals().get("other_root"), Some(&Value::Float(5.0)));
    assert_eq!(vm.globals().get("rounded"), Some(&Value::Int(4)));
    assert_eq!(vm.globals().get("absolute"), Some(&Value::Int(5)));
    assert_eq!(vm.globals().get("sine"), Some(&Value::Float(0.0)));
    assert_eq!(vm.globals().get("cosine"), Some(&Value::Float(1.0)));
    assert_eq!(vm.globals().get("tangent"), Some(&Value::Float(0.0)));
    assert_eq!(vm.globals().get("floored"), Some(&Value::Int(2)));
    assert_eq!(vm.globals().get("ceiled"), Some(&Value::Int(3)));
    assert_eq!(vm.globals().get("minimum"), Some(&Value::Int(2)));
    assert_eq!(vm.globals().get("maximum"), Some(&Value::Int(3)));
    assert_eq!(vm.globals().get("power"), Some(&Value::Float(16.0)));
    assert_eq!(vm.globals().get("uppercase"), Some(&Value::from("NOVA")));
    assert_eq!(vm.globals().get("lowercase"), Some(&Value::from("nova")));
    assert_eq!(vm.globals().get("trimmed"), Some(&Value::from("nova")));
    assert_eq!(vm.globals().get("string_has"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals().get("replaced"), Some(&Value::from("newa")));
    assert_eq!(vm.globals().get("joined"), Some(&Value::from("a-b-c")));
    assert_eq!(vm.globals().get("length"), Some(&Value::Int(3)));
    assert_eq!(vm.globals().get("contains"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals().get("dictionary_has"), Some(&Value::Bool(true)));
    assert_eq!(
        vm.globals().get("keys"),
        Some(&Value::List(vec![Value::from("volume")]))
    );
    assert_eq!(
        vm.globals().get("values"),
        Some(&Value::List(vec![Value::Int(5)]))
    );
    assert_eq!(
        vm.output(),
        &["9 5 4 NOVA a-b-c 3 true [volume] 3.141592653589793".to_string()]
    );
}

#[test]
fn built_in_time_module_exposes_clock_and_sleep_functions() {
    let vm = run_source(
        "get Time\nfrom Time get unix\nstamp = unix()\nprecise = Time.now()\nTime.sleep(0)\nTime.sleep_ms(0)\nstop",
    );
    assert!(matches!(vm.globals().get("stamp"), Some(Value::Int(value)) if *value > 0));
    assert!(matches!(vm.globals().get("precise"), Some(Value::Float(value)) if *value > 0.0));
}

#[test]
fn from_get_imports_bind_callable_module_items() {
    let vm = run_source(
        "from String get upper\nfrom Collections get len\nlabel = upper(\"nova\")\nsize = len([label])\nprint(label, size)\nstop",
    );
    assert_eq!(vm.globals().get("label"), Some(&Value::from("NOVA")));
    assert_eq!(vm.globals().get("size"), Some(&Value::Int(1)));
    assert_eq!(vm.output(), &["NOVA 1".to_string()]);
}

#[test]
fn built_in_modules_report_domain_and_unknown_module_errors() {
    let chunk = compile_source("get Math\nvalue = Math.sqrt(-1)\nstop").unwrap();
    assert!(
        Vm::new(chunk)
            .run()
            .unwrap_err()
            .to_string()
            .contains("nonnegative")
    );

    let chunk = compile_source("get Missing\nstop").unwrap();
    assert!(
        Vm::new(chunk)
            .run()
            .unwrap_err()
            .to_string()
            .contains("Unknown module")
    );
}

#[test]
fn arithmetic_stack_and_variable_instructions_execute() {
    let mut chunk = Chunk::new();
    let c10 = chunk.add_constant(Value::Int(10));
    let c3 = chunk.add_constant(Value::Int(3));
    let c2 = chunk.add_constant(Value::Int(2));
    let c1 = chunk.add_constant(Value::Int(1));
    let ctrue = chunk.add_constant(Value::Bool(true));
    let cfalse = chunk.add_constant(Value::Bool(false));

    chunk.write(Instruction::LoadConstant(c10));
    chunk.write(Instruction::LoadConstant(c3));
    chunk.write(Instruction::Modulo);
    chunk.write(Instruction::StoreVariable("remainder".to_string()));

    chunk.write(Instruction::LoadConstant(c2));
    chunk.write(Instruction::LoadConstant(c3));
    chunk.write(Instruction::Power);
    chunk.write(Instruction::StoreVariable("power".to_string()));

    chunk.write(Instruction::LoadConstant(c1));
    chunk.write(Instruction::Duplicate);
    chunk.write(Instruction::Add);
    chunk.write(Instruction::DefineVariable("doubled".to_string()));

    chunk.write(Instruction::LoadConstant(c10));
    chunk.write(Instruction::LoadConstant(c3));
    chunk.write(Instruction::Swap);
    chunk.write(Instruction::Subtract);
    chunk.write(Instruction::StoreVariable("difference".to_string()));

    chunk.write(Instruction::LoadConstant(ctrue));
    chunk.write(Instruction::LoadConstant(cfalse));
    chunk.write(Instruction::And);
    chunk.write(Instruction::StoreVariable("and_result".to_string()));

    chunk.write(Instruction::LoadConstant(cfalse));
    chunk.write(Instruction::LoadConstant(ctrue));
    chunk.write(Instruction::Or);
    chunk.write(Instruction::StoreVariable("or_result".to_string()));

    chunk.write(Instruction::LoadNone);
    chunk.write(Instruction::DefineVariable("temporary".to_string()));
    chunk.write(Instruction::LoadVariable("temporary".to_string()));
    chunk.write(Instruction::Not);
    chunk.write(Instruction::StoreVariable("none_is_false".to_string()));
    chunk.write(Instruction::DeleteVariable("temporary".to_string()));
    chunk.write(Instruction::Stop);

    let mut vm = Vm::new(chunk);
    vm.run().unwrap();
    assert_eq!(vm.globals().get("remainder"), Some(&Value::Int(1)));
    assert_eq!(vm.globals().get("power"), Some(&Value::Int(8)));
    assert_eq!(vm.globals().get("doubled"), Some(&Value::Int(2)));
    assert_eq!(vm.globals().get("difference"), Some(&Value::Int(-7)));
    assert_eq!(vm.globals().get("and_result"), Some(&Value::Bool(false)));
    assert_eq!(vm.globals().get("or_result"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals().get("none_is_false"), Some(&Value::Bool(true)));
    assert!(!vm.globals().contains_key("temporary"));
}
