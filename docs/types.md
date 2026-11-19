# Nova Types and Runtime Values

## Confirmed language design

Nova variables are fully dynamic. Assigning a new value replaces the previous
value and may change its type without an explicit declaration or conversion.

The source-level types are:

| Type | Example |
|---|---|
| `Int` | `42` |
| `Float` | `3.5` |
| `String` | `"hello"` |
| `Bool` | `true` |
| `List` | `[1, "two"]` |
| `Dictionary` | `{"active": true}` |

The runtime also uses an internal `None` value for operations that produce no
value. It is not currently a source literal.

## Current execution behavior

The compiler and VM support dynamic variables, expressions, functions,
conditionals, Nova's loop forms, printing, and list/dictionary construction,
indexing, and mutation. VM print output is captured one line at a time for the
host or CLI. The following operator details describe the implementation today;
they do not add language guarantees beyond the specification:

- Integer addition, subtraction, and multiplication keep an integer result and
  report overflow as a runtime error.
- Mixed integer/float arithmetic produces a float. Division produces a float
  and reports division by zero. Non-finite float literals, operations, and
  conversions are rejected.
- `+` concatenates two strings. Other unsupported operand combinations report
  a runtime error.
- Equality compares values of the same runtime type; `1` and `1.0` are not
  equal. Dictionary equality ignores insertion order, resolves duplicate keys
  with last-value-wins semantics, and compares nested keys and values
  recursively. Ordered comparisons accept numeric values and preserve integer
  precision when comparing an `Int` with a `Float`.
- Logical operations use truthiness: zero, an empty string, an empty
  collection, and `None` are false; non-zero numbers and non-empty values are
  true. Non-finite floats cannot be converted to `Bool`.
- `and` and `or` evaluate the right operand only when needed.

List indexes are zero-based, nonnegative integers and must refer to an existing
element. Dictionary reads require an existing key, while assignment creates a
missing key. Nested reads and assignments are supported. `int()`, `float()`,
`str()`, and `bool()` are available as one-argument conversion functions;
conversion failures produce runtime errors. `int(Float)` truncates toward
zero. `float(String)` accepts only finite numeric text; `int(String)` requires
an integer string. Converting `true`/`false` to numbers yields `1`/`0` (or
`1.0`/`0.0`).

The public AST-based `Runtime` helper evaluates a smaller subset of the
language; the bytecode compiler and VM are the CLI's complete execution path.
