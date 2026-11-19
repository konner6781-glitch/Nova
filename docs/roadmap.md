# Nova Implementation Roadmap

This page distinguishes executable features from experimental features and
future design goals.

## Implemented

- Rust lexer, indentation-aware parser, AST, bytecode compiler, and VM used by
  the command-line runner.
- Top-level `stop` validation and source locations carried through compiled
  instructions to runtime diagnostics.
- Dynamic values, numeric and string expressions, comparisons, short-circuit
  boolean operators, printing, and the four conversion functions.
- `if` / `elif` / `else`; Nova's continuous and repeat loops, delays, iteration
  limits, and innermost-loop `break`.
- Top-level functions, parameters, local scopes without implicit global
  capture, recursion, zero/one/multiple returns, and unpack assignment.
- Lists and dictionaries, nested indexing, assignment, compound assignment,
  dictionary key insertion, and built-in collection operations.
- Built-in `Math`, `Time`, `String`, and `Collections` modules through `get`
  and `from … get`.
- Rust VM API for registering host-provided module values.
- CLI source-file execution with path-aware diagnostics.
- Automated tests and Windows/Linux CI for core builds.

## Implemented but experimental

- The `UI` module models native windows, components, controls, callbacks,
  themes, and updates. Its desktop backend is Windows-only and needs the
  Microsoft Edge WebView2 Evergreen Runtime. UI programs/examples are covered
  by model and VM tests, but native window behavior still needs manual smoke
  testing on supported Windows installations.
- `runtime.rs` retains a partial AST evaluator as a public helper/reference
  path. It is not the CLI runtime and does not implement the VM's full control
  flow, module, function, or UI behavior. Use the bytecode compiler and VM for
  complete Nova program execution.

## Planned

- Loading user-created `.nova` source modules, with search paths, caching,
  circular-import handling, and import diagnostics.
- More standard-library modules (including file, system, network, and process
  APIs) with explicit capability and error behavior.
- Classes and object-oriented features, designed separately from the current
  function model.
- A stable release process, end-user installers, `PATH` configuration,
  WebView2 detection/guidance, packaged examples, and update/uninstall flows.
- Performance baselines and profiling before optimization work.
- UI backend improvements, broader test coverage of native lifecycle failures,
  and platform decisions beyond Windows.

## Versioning

`Cargo.toml` currently identifies the package as `0.1.0`. The language
specification's `0.2` label identifies a design-document revision; it is not a
package release number.

## Verification

Run `cargo fmt --all -- --check`, `cargo test --locked`,
`cargo clippy --all-targets --all-features --locked -- -D warnings`, and
`cargo build --locked`. GitHub Actions runs these checks on Windows and Linux.
