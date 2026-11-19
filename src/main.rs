use std::env;
use std::fs;
use std::path::PathBuf;

use nova::compiler;
use nova::lexer::Lexer;
use nova::parser::Parser;
use nova::vm::Vm;

fn main() {
    if let Err(error) = run() {
        eprintln!("Nova error: {}", error);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut arguments = env::args_os().skip(1);
    let Some(path) = arguments.next() else {
        return Err("Usage: nova <file.nova>".into());
    };
    if arguments.next().is_some() {
        return Err("Usage: nova <file.nova>".into());
    }

    let path = PathBuf::from(path);
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("Cannot read '{}': {}", path.display(), error))?;

    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|error| format!("{}: Lexer: {}", path.display(), error))?;
    let mut parser = Parser::new(tokens);
    let program = parser
        .parse()
        .map_err(|error| format!("{}: Parser: {}", path.display(), error))?;
    let chunk = compiler::compile(&program)
        .map_err(|error| format!("{}: Compiler: {}", path.display(), error))?;

    let mut vm = Vm::new(chunk);
    vm.run()
        .map_err(|error| format!("{}: Runtime: {}", path.display(), error))?;
    for line in vm.take_output() {
        println!("{}", line);
    }

    Ok(())
}
