
use nova::lexer::Lexer;
use nova::parser::Parser;

fn main() {
    println!("Nova 0.1.0");
    println!("Testing Nova lexer and parser...\n");

    let source = r#"
score = 100
name = "Nova"

if score > 50:
    print("You passed!")
else:
    print("Try again!")

stop
"#;

    // Step 1: Convert source code into tokens.
    let mut lexer = Lexer::new(source);

    let tokens = match lexer.tokenize() {
        Ok(tokens) => tokens,
        Err(error) => {
            println!("Lexer error: {}", error);
            return;
        }
    };

    // Step 2: Convert tokens into an AST.
    let mut parser = Parser::new(tokens);

    let program = match parser.parse() {
        Ok(program) => program,
        Err(error) => {
            println!("Parser error: {}", error);
            return;
        }
    };

    // Step 3: Display the AST.
    println!("Parsing successful!\n");
    println!("{:#?}", program);
}
