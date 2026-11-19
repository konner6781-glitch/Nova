# Nova Syntax Reference

Every Nova program ends with one top-level stop statement:

    print("Hello from Nova")
    stop

## Variables and expressions

Variables are dynamic and can change type when reassigned:

    score = 10
    score = "ten"
    total = 2 + 3 * 4
    active = score == "ten" and not false

Supported expression operators are `+`, `-`, `*`, `/`, the comparison
operators, and the word operators `and`, `or`, and `not`. `and` and `or`
short-circuit and return `Bool` values. The VM handles modulo and power
instructions supplied in bytecode, but `%` and `^` are not Nova source
operators and the compiler does not emit them.

Strings support `\n`, `\t`, `\r`, `\\`, `\"`, and `\'`. Unsupported
escapes and unescaped newlines produce lexer errors. Escapes decode once.

Integer arithmetic preserves `Int` and errors on overflow. Mixed numeric
operations return `Float`; division always returns `Float` and errors on zero.
Float literals and results must be finite. `int(Float)` truncates toward zero;
`int(String)` parses an integer string, and `float(String)` accepts only finite
numeric text.

The conversion functions each take one value:

    whole = int("12")
    ratio = float(3)
    label = str(true)
    enabled = bool([])

## Conditions

    if score > 90:
        print("A")
    elif score >= 70:
        print("B")
    else:
        print("Keep practicing")

## Loops

    loop:
        print("Runs until break")
        break

    loop.repeat 3, delay(0):
        print("Counted repetition")

    loop.repeat score < 10, 5:
        score += 1

    loop.repeat score < 10; 5, delay(0):
        score += 1

`loop.repeat` treats a Boolean expression as a condition and a numeric
expression as a repeat count. A maximum count and a delay are optional.
`break` exits the innermost active loop.

## Functions

Functions are declared at the top level. Their variables are local, and they
do not implicitly capture global variables:

    function add(a, b):
        return(a + b)

    answer = add(5, 10)

Functions may return multiple values:

    function pair():
        return(10, 20)

    first, second = pair()
    values = pair()

When multiple values are stored in one variable, Nova represents them as a
list.

## Collections

List and dictionary literals support nested indexing and mutation:

    items = [1, "two", true]
    settings = {"volume": 5, "active": true}
    first = items[0]
    items[1] = "changed"
    settings["volume"] += 1

List indexes are zero-based nonnegative integers and must refer to an existing
element. Reading a missing dictionary key is an error; assigning a missing key
adds it. Indexed assignments can target nested lists and dictionaries.
Dictionary keys can be any Nova value. Duplicate keys keep the last assigned
value, and dictionary equality ignores insertion order while comparing key and
value types strictly.

## Imports

Nova uses `get` for imports:

    get Math
    result = Math.sqrt(25)

    from String get upper
    label = upper("nova")

Built-in modules include `Math`, `Time`, `String`, `Collections`, and `UI`.
Nova source files are not loaded as modules yet; a Rust host can also register
custom value exports with the VM. `UI` opens native desktop windows on Windows;
see [the UI guide](ui.md) for its event loop and WebView2 requirement.
