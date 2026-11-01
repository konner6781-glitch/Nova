# Nova — Language Specification

**Version:** 0.2 — Language Design  
**Status:** In Development  
**Language:** Nova

---

# 1. Language Overview

Nova is a general-purpose programming language designed to be readable, powerful, and efficient.

Nova aims to combine the readability of beginner-friendly languages with the capabilities and flexibility of more advanced programming languages.

### Core Goals

- Simple and readable syntax.
- Extensive built-in functionality.
- Easy creation and use of libraries.
- Flexible functions and variable handling.
- Efficient program execution.
- Support for interpretation and eventual compilation.
- An approachable language for beginners without unnecessarily limiting experienced developers.

Nova takes inspiration from languages such as Python, C++, and Luau while developing its own syntax and features.

# 2. Variables and Data Types

## 2.1 Variable Declaration

Nova automatically determines a variable's initial data type based on the assigned value.

Variables do not require an explicit type declaration.

Example:

```nova
age = 15
username = "NovaUser"
temperature = 24.5
is_active = true
```

## 2.2 Built-in Data Types

Nova initially supports four fundamental data types:

| Type | Description | Example |
|---|---|---|
| `Int` | Whole numbers | `15` |
| `Float` | Decimal numbers | `15.5` |
| `String` | Text | `"Hello"` |
| `Bool` | True or false values | `true` |

## 2.3 Variable Reassignment

Variables can be reassigned as long as the new value matches their existing data type.

```nova
x = 15
x = 20
```

Changing a variable's data type requires an explicit conversion.

```nova
x = 15
x = str("hello")
```

After conversion, `x` becomes a String variable.

Assigning a different data type without explicitly converting it must produce a type error.

## 2.4 Type Conversion

Nova provides four lowercase conversion functions:

- `int()` — converts a value to an integer.
- `float()` — converts a value to a float.
- `str()` — converts a value to a string.
- `bool()` — converts a value to a boolean.

Example:

```nova
age = 15
age = str("fifteen")
```

Conversion functions must use lowercase names.

# 3. Program Termination

Every Nova program must end with the `stop` keyword.

The keyword marks the end of the program and must appear exactly once.

Example:

```nova
print("Hello, World!")

stop
```

### Termination Rules

1. Every program must contain `stop`.
2. `stop` must be the final statement.
3. `stop` cannot appear more than once.
4. No statements may appear after `stop`.
5. `stop` is a reserved keyword and cannot be used as a variable or function name.

The interpreter must validate these rules before executing the program.

# 4. Code Blocks and Indentation

Nova uses indentation to organize code blocks.

Curly braces are not used to define code blocks.

A colon (`:`) marks the beginning of a block, while indentation determines which statements belong to it.

## 4.1 Flexible Indentation

Nova allows indentation levels of 1–4 spaces.

Both examples below are valid:

```nova
if x == 15:
 print("x is 15")
```

```nova
if x == 15:
    print("x is 15")
```

Indentation must remain consistent within each block. Inconsistent indentation that makes the structure ambiguous must produce a syntax error.

Indentation is handled during source-code parsing and should not create unnecessary runtime overhead.

# 5. Conditional Statements

Nova supports three conditional keywords:

- `if`
- `elif`
- `else`

These statements control which sections of code execute based on conditions.

Every conditional header must end with a colon (`:`).

## 5.1 If

Checks whether a condition is true.

```nova
if x == 15:
    print("x is 15")
```

## 5.2 Elif

Checks another condition if the preceding condition was false.

```nova
if x == 15:
    print("x is 15")
elif x == 20:
    print("x is 20")
```

## 5.3 Else

Executes when none of the preceding conditions were true.

```nova
if x == 15:
    print("x is 15")
elif x == 20:
    print("x is 20")
else:
    print("x is something else")
```

## 5.4 Conditional Rules

1. Keywords must be lowercase.
2. Conditional headers must end with a colon.
3. `elif` must follow an `if` or another `elif`.
4. `else` must follow an `if` or `elif`.
5. `else` cannot have a condition.
6. Indentation determines which statements belong to each conditional block.

# 6. Loop System

Nova uses a custom loop system built around the `loop` keyword.

There are two primary loop forms: continuous loops and repeat loops.

## 6.1 Continuous Loop

The `loop:` statement creates a continuous loop that repeats indefinitely until explicitly exited.

Example:

```nova
loop:
    print("Running...")
```

The loop continues until a loop-exit statement is encountered.

## 6.2 Counted Repetition

`loop.repeat` followed by a number repeats the code block exactly that many times.

Example:

```nova
loop.repeat 10:
    print("Hello")
```

This executes the block ten times.

## 6.3 Conditional Repetition

`loop.repeat` can also accept a condition.

Example:

```nova
x = 0

loop.repeat x < 10:
    print(x)
    x = x + 1
```

The condition is checked before every iteration.

The loop continues while the condition remains true.

## 6.4 Loop Delays

Nova supports an optional delay argument for repeated loops.

Example:

```nova
loop.repeat 10, delay(0.01):
    print("Hello")
```

The delay pauses execution between repetitions.

The delay is measured in seconds.

The first iteration begins immediately; delays occur between iterations rather than before the first iteration.

## 6.5 Loop Rules

1. `loop:` creates an unconditional continuous loop.
2. `loop.repeat` accepts either a numeric repetition count or a condition.
3. Numeric repetition executes the block the specified number of times.
4. Conditional repetition checks its condition before each iteration.
5. An optional delay can be supplied to `loop.repeat`.
6. Loop bodies use indentation.
7. A loop-exit mechanism will be provided to stop loops.

# 7. Functions

Nova uses the `function` keyword to define reusable blocks of code.

Functions can receive parameters, perform operations, and return values to the code that called them.

## 7.1 Function Declaration

Example:

```nova
function greet():
    print("Hello!")
```

The `function` keyword defines a function.

The function name identifies it when it is called.

Function bodies use indentation, just like other Nova code blocks.

## 7.2 Function Parameters

Functions can receive information through parameters.

Example:

```nova
function greet(name):
    print("Hello " + name)

greet("NovaUser")
```

In this example:

- `name` is the parameter.
- `"NovaUser"` is the argument.
- The function receives the argument and uses it internally.

Functions can accept multiple parameters.

Example:

```nova
function multiply(a, b, c):
    result = a * b * c
    return(result)

answer = multiply(2, 3, 4)
```

## 7.3 Returning Values

Functions can send information back to the code that called them using `return`.

Example:

```nova
function add(a, b):
    result = a + b
    return(result)

answer = add(10, 20)
```

The returned value can be assigned to a variable or used in another expression.

## 7.4 Multiple Return Values

Nova supports returning multiple values from a single function.

Example:

```nova
function add(a, b):
    equation = str(a) + " + " + str(b)
    result = a + b
    rounded_result = round(result, 2)

    return(equation, result, rounded_result)
```

A function can return one value, multiple values, or no values.

Different return statements within the same function may return different numbers of values.

Example:

```nova
function add(a, b):
    result = a + b

    if result > 100:
        return("Error: Result exceeds 100")

    return(result)
```

The function does not need to return placeholder values for results it does not need.

## 7.5 Receiving Multiple Return Values

Nova supports two ways of receiving multiple returned values.

**Method 1: Unpacking into separate variables**

```nova
equation, result, rounded = add(10, 20)
```

Each returned value is assigned to its corresponding variable.

**Method 2: Keeping returned values together**

```nova
results = add(10, 20)
```

The returned values are kept together in one variable.

Both approaches are supported.

## 7.6 Function Scope

Variables created inside functions are local to those functions.

They cannot automatically be accessed outside the function.

Example:

```nova
function calculate():
    result = 25

calculate()

print(result)
```

This must produce an error because `result` only exists inside `calculate()`.

Information can leave a function through explicitly returned values.

Functions receive external values through their parameters.

Functions should not automatically access variables from outside their own scope. External values should be passed through parameters.

## 7.7 Function Rules

1. Functions are declared using the lowercase `function` keyword.
2. Functions can accept zero or more parameters.
3. Functions can return zero, one, or multiple values.
4. `return` immediately exits the function.
5. Returned values are available to the caller.
6. Variables created inside functions are local.
7. Functions use indentation for their bodies.

# 8. Standard Library

Nova is intended to provide extensive built-in functionality through its standard library.

Planned modules include:

| Module | Intended Purpose |
|---|---|
| `Files` | File creation, reading, writing, and management |
| `Program` | Program and application information |
| `System` | Operating system functionality |
| `Network` | Networking functionality |
| `UI` | User interface creation |
| `Math` | Mathematical operations |
| `Time` | Time and date functionality |
| `Collections` | Lists, dictionaries, and other collections |
| `String` | Text manipulation |

These are planned modules and may not all be available in the initial implementation.

# 9. Future Development

Nova will initially be developed as an interpreted language.

The initial implementation will focus on:

- Lexer and token recognition.
- Parser and syntax validation.
- Interpreter and program execution.
- Fundamental data types.
- Variables and type conversion.
- Conditional statements.
- Loops.
- Functions and return values.
- Standard library foundations.

Future development may include:

- Compilation support.
- Third-party libraries.
- Native extensions.
- Advanced data structures.
- Object-oriented programming.
- Additional performance optimizations.

The specification will evolve as additional language-design decisions are made.

---

**Nova — Built for the future.**