# Nova

Nova is a dynamically typed, indentation-based programming language implemented
in Rust. Its primary execution path compiles source into bytecode and runs it in
Nova's virtual machine. Nova keeps its own syntax and loop design; it is not a
Python implementation.

## Project status

Nova is an early development project, not yet a stable release. Cargo currently
labels the package `0.1.0`; the language specification's `0.2` label is a design
document revision, not a released package version. The compiler, VM, built-in
libraries, and Windows UI path are implemented and covered by automated tests.
Source-file imports, classes, native extensions, and packaged installers are
not implemented.

## Build and run

Building from source requires Rust and Cargo 1.85 or newer.

```text
cargo build
cargo run -- examples/hello.nova
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

The CLI accepts a source file path directly, including paths with spaces. Each
program must end with exactly one top-level `stop` statement.

## Implemented language features

- Dynamic variables; `Int`, `Float`, `String`, `Bool`, `List`, and `Dictionary`
  values.
- Arithmetic, comparisons, short-circuit `and` / `or`, and `not`.
- `if` / `elif` / `else`, Nova's `loop` and `loop.repeat` forms, delays, loop
  limits, and `break`.
- Top-level functions, local scopes, recursion, zero/one/multiple returns, and
  multiple-value unpacking.
- Nested list and dictionary indexing, assignment, and compound assignment.
- `int()`, `float()`, `str()`, and `bool()` conversions.
- Built-in `Math`, `Time`, `String`, and `Collections` modules imported with
  Nova's `get` syntax.
- A built-in `UI` module for experimental Windows desktop applications.

See [the syntax reference](docs/syntax.md),
[language specification](docs/language_specification.md), and
[standard library reference](docs/standard_library.md).

## Supported systems and UI requirement

The core language is intended to build on Windows and Linux; CI checks both.
The desktop UI backend is currently Windows-only and requires the Microsoft
Edge WebView2 Evergreen Runtime. The renderer uses the runtime bundled in Nova,
so Node.js and npm are not needed. See [the UI guide](docs/ui.md).

List indexes are zero-based, nonnegative, and must reference existing elements.
Reading a missing dictionary key is an error; assigning a missing key creates
it. Functions do not capture global variables or module bindings; import a
module inside a function when it is used there.

## Development and future distribution

Run `cargo test` for lexer, parser, compiler, VM, standard-library, UI model,
and integration coverage. GitHub Actions runs formatting, tests, Clippy, and
builds on Windows and Linux.

Nova does not yet provide an end-user installer or add itself to `PATH`.
Creating release packages, configuring `PATH`, detecting or installing
WebView2, and providing update/uninstall flows remain distribution work.
Rust hosts can register module values through the VM API; source `.nova`
modules and native extensions are future work.
