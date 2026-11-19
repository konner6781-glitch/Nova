use nova::bytecode::Instruction;
use nova::compiler;
use nova::lexer::Lexer;
use nova::parser::Parser;

fn compile_source(source: &str) -> Result<nova::bytecode::Chunk, String> {
    let tokens = Lexer::new(source).tokenize()?;
    let program = Parser::new(tokens).parse()?;
    compiler::compile(&program)
}

#[test]
fn compiler_emits_source_mapped_constants_arithmetic_and_store() {
    let chunk = compile_source("answer = 10 / 2\nstop").unwrap();
    let division = chunk
        .instructions
        .iter()
        .position(|instruction| *instruction == Instruction::Divide)
        .unwrap();
    assert!(matches!(
        chunk.instructions[0],
        Instruction::LoadConstant(_)
    ));
    assert!(matches!(
        chunk.instructions[1],
        Instruction::LoadConstant(_)
    ));
    assert_eq!(
        chunk.instructions[division + 1],
        Instruction::StoreVariable("answer".into())
    );
    assert_eq!(
        chunk.source_location(division),
        Some(nova::tokens::SourceLocation {
            line: 1,
            column: 13
        })
    );
}

#[test]
fn compiler_patches_branches_and_loop_jumps_inside_the_chunk() {
    let chunk = compile_source(
        "value = 0\nif true:\n    value = 1\nelse:\n    value = 2\nloop.repeat value < 4, 3:\n    value += 1\nstop",
    )
    .unwrap();
    let length = chunk.instruction_count();
    for instruction in &chunk.instructions {
        match instruction {
            Instruction::Jump(target)
            | Instruction::JumpIfFalse(target)
            | Instruction::JumpIfTrue(target)
            | Instruction::LoopCheck(target)
            | Instruction::LoopLimitCheck(target)
            | Instruction::Break(target) => assert!(*target < length),
            Instruction::LoopEnd(target) => assert!(*target < length),
            _ => {}
        }
    }
    assert!(
        chunk
            .instructions
            .iter()
            .any(|item| matches!(item, Instruction::JumpIfFalse(_)))
    );
    assert!(
        chunk
            .instructions
            .iter()
            .any(|item| matches!(item, Instruction::LoopStart))
    );
    assert!(
        chunk
            .instructions
            .iter()
            .any(|item| matches!(item, Instruction::LoopLimitCheck(_)))
    );
}

#[test]
fn compiler_registers_function_boundaries_and_implicit_return() {
    let chunk =
        compile_source("function add(a, b):\n    return(a + b)\nvalue = add(1, 2)\nstop").unwrap();
    let (entry, end) = chunk
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::DefineFunction {
                entry_point,
                end_point,
                parameters,
                ..
            } => {
                assert_eq!(parameters, &["a", "b"]);
                Some((*entry_point, *end_point))
            }
            _ => None,
        })
        .unwrap();
    assert!(entry < end);
    assert!(end <= chunk.instruction_count());
    assert!(
        chunk.instructions[entry..end]
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Return { value_count: 1 }))
    );
}

#[test]
fn compile_errors_include_the_statement_source_location() {
    let error = compile_source("return(1)\nstop").unwrap_err();
    assert!(error.contains("return"));
    assert!(error.contains("line 1, column 1"));
}
