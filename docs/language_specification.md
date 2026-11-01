# Nova Language Specification

## 1. Language Overview

Nova is a general-purpose programming language designed to be simple to read, powerful to use, and easy to extend.

The language aims to provide a straightforward syntax while including a wide range of built-in functionality. Nova also aims to make creating and using libraries accessible to developers.

Nova is designed to combine the readability of Python with the structure and capabilities found in languages such as C++.

## 2. Variables and Data Types

### 2.1 Variable Declaration

Variables in Nova do not require an explicit type declaration. Instead, Nova automatically determines a variable's type based on the value assigned to it.

For example:

- `x = 15` creates an integer variable.
- `name = "Nova"` creates a string variable.
- `price = 12.5` creates a floating-point variable.
- `active = true` creates a boolean variable.

Variables can be reassigned as long as the new value matches their current type.

### 2.2 Supported Data Types

Nova initially supports the following fundamental data types:

- **Int:** Whole numbers.
- **Float:** Numbers containing decimal values.
- **String:** Text.
- **Bool:** Boolean values (`true` or `false`).

Additional data types may be introduced as the language develops.

### 2.3 Explicit Type-Changing Assignment

Nova allows variables to change their data type, but changing a variable's type requires an explicit conversion.

This means a variable can be reassigned normally when the new value matches its existing type. However, assigning a value of a different type without explicitly converting it will produce an error.

To change a variable's type, developers must use one of Nova's built-in conversion functions:

- `int()`
- `float()`
- `str()`
- `bool()`

For example:

- `x = 15` creates an integer variable.
- `x = 20` is allowed because the value is still an integer.
- `x = str("hello")` changes the variable's type to String.
- `x = "world"` is now allowed because the variable is already a String.

However, assigning an integer to `x` again without converting it will produce an error.

After an explicit conversion, Nova updates the variable's stored type to match the converted value.

## 3. Program Termination

### 3.1 The Stop Keyword

Every Nova program must end with the `stop` keyword.

The `stop` keyword marks the official end of a program and provides the interpreter with a clear termination point.

The following rules apply:

- Every program must contain exactly one `stop` keyword.
- `stop` must be the final statement in the program.
- No statements may appear after `stop`.
- Using `stop` more than once is not allowed.
- `stop` is a reserved keyword and cannot be used as a variable or function name.

If any of these rules are violated, Nova must report an appropriate error.

## 4. Syntax

This section will define Nova's syntax rules, including statements, expressions, indentation, functions, conditions, loops, and other language structures.

## 5. Standard Library

Nova is intended to include a comprehensive standard library containing built-in modules for common programming tasks.

The planned modules include:

- Files
- Program
- System
- Network
- UI
- Math
- Time
- Collections
- String

The exact functionality of each module will be defined as Nova develops.

## 6. Future Development

Nova will initially be developed as an interpreted language. Future development may introduce compilation alongside interpretation.

Additional language features, data types, libraries, and development tools will be introduced over time.