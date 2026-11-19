# Nova Standard Library

Nova's VM and CLI provide these built-in modules. Import syntax uses Nova's
`get` keyword:

```nova
get Math
root = Math.sqrt(81)

from Math get sqrt
root = sqrt(81)
stop
```

Function calls with the wrong number or type of arguments report runtime
errors. Numeric library functions reject non-finite inputs and results.

## Math

Trigonometric functions use radians. `round` follows Rust's `f64::round`
behavior (half away from zero). Integer-returning operations report an error
when their result is outside Nova's `Int` range.

| Name | Arguments | Return | Behavior and errors |
|---|---|---|---|
| `Math.pi`, `Math.e`, `Math.tau` | none | `Float` | Constants for π, e, and τ. |
| `Math.abs(value)` | one `Int` or finite `Float` | same numeric type | Absolute value; errors for `Int` minimum overflow or invalid type. |
| `Math.sqrt(value)` | one finite number ≥ 0 | `Float` | Square root; negative inputs are errors. |
| `Math.sin(value)`, `Math.cos(value)`, `Math.tan(value)` | one finite number | `Float` | Trigonometric result in radians; non-finite result is an error. |
| `Math.floor(value)`, `Math.ceil(value)`, `Math.round(value)` | one finite number | `Int` | Round down, up, or to nearest; out-of-range result is an error. |
| `Math.min(left, right)`, `Math.max(left, right)` | two finite numbers | `Int` if both inputs are `Int`, otherwise `Float` | Return the smaller or larger value. |
| `Math.pow(base, exponent)` | two finite numbers | `Float` | Exponentiation; invalid/non-finite results are errors. |

```nova
get Math
rounded = Math.round(-2.5)
distance = Math.sqrt(3 * 3 + 4 * 4)
stop
```

## Time

All durations are blocking: the current Nova VM thread pauses during sleep.

| Name | Arguments | Return | Behavior and errors |
|---|---|---|---|
| `Time.now()` | none | `Float` | Current Unix time in seconds, including fractional seconds. |
| `Time.unix()` | none | `Int` | Current Unix time in whole seconds. |
| `Time.sleep(seconds)` | one finite, nonnegative number | `None` | Pauses for seconds; rejects durations too large for the platform. |
| `Time.sleep_ms(milliseconds)` | one finite, nonnegative number | `None` | Pauses for milliseconds; rejects durations too large for the platform. |

```nova
get Time
started = Time.now()
Time.sleep_ms(10)
elapsed = Time.now() - started
stop
```

## String

Strings are Unicode. `upper` and `lower` use Rust's Unicode case conversion;
`split` requires a nonempty separator and preserves empty fields between or at
the ends of separators. `join` converts each list element with Nova's `str`
display conversion.

| Name | Arguments | Return | Behavior and errors |
|---|---|---|---|
| `String.upper(text)`, `String.lower(text)` | one `String` | `String` | Unicode uppercase/lowercase conversion. |
| `String.trim(text)` | one `String` | `String` | Removes leading and trailing Unicode whitespace. |
| `String.contains(text, search)` | two `String`s | `Bool` | Checks for a substring. |
| `String.replace(text, from, to)` | three `String`s | `String` | Replaces every match of `from` with `to`; an empty `from` follows Rust string replacement behavior. |
| `String.split(text, separator)` | two `String`s | `List[String]` | Splits on a nonempty exact substring; empty separators are errors. |
| `String.join(separator, parts)` | `String`, `List` | `String` | Joins each part's string representation with the separator; non-list second arguments are errors. |

```nova
get String
words = String.split("red,green", ",")
label = String.join(" / ", words)
stop
```

## Collections

| Name | Arguments | Return | Behavior and errors |
|---|---|---|---|
| `Collections.len(value)` | one `String`, `List`, or `Dictionary` | `Int` | Counts Unicode scalar values in strings, elements in lists, or unique keys in dictionaries. Other types are errors. |
| `Collections.contains(list, value)` | `List`, any value | `Bool` | Checks list elements using Nova's type-sensitive equality. |
| `Collections.contains(dictionary, key)` | `Dictionary`, any value | `Bool` | Checks for a key. |
| `Collections.keys(dictionary)` | one `Dictionary` | `List` | Returns unique keys in first-insertion order. |
| `Collections.values(dictionary)` | one `Dictionary` | `List` | Returns values in key order; duplicate keys keep their last value. |

Dictionary equality ignores insertion order. Duplicate keys supplied by host
code are canonicalized with last-value-wins behavior while preserving the
first position of each key. Nova source dictionary assignment follows the same
last-value-wins behavior.

```nova
get Collections
count = Collections.len("Nova")
has_key = Collections.contains({"enabled": true}, "enabled")
stop
```

## UI

`UI` provides experimental desktop windows, layouts, controls, callbacks, and
live updates. Its full function signatures, return handles, errors, styles,
and examples are in [the UI guide](ui.md). It requires Windows and the
Microsoft Edge WebView2 Evergreen Runtime.

## Host modules and source imports

A Rust application embedding Nova can register module values with the VM's
`register_module` API. `get` currently imports built-in or host-registered
modules; it does not load `.nova` files. Source modules, network, and process
modules remain future work.

## Files

`get Files` exposes synchronous filesystem operations using host path rules.
`read(path)` returns UTF-8 text; `read_bytes(path)` returns byte values as
`List[Int]`; `read_lines(path)` returns lines. `write(path, text)` overwrites,
`append(path, text)` appends, and `write_bytes(path, bytes)` writes bytes.
`exists`, `is_file`, `size`, `extension`, `name`, `parent`, and `info` inspect
paths. `copy`, `move`, and `rename` refuse existing destinations. `create`
refuses to replace an existing file. `delete` removes a file only. I/O failures
include the path and operating-system error. Reads currently load the file into
memory.

## Directories

`get Directories` provides `create(path)` for one new directory,
`create_all(path)` for recursive creation, `delete(path)` for an empty
directory, and explicitly recursive `delete_all(path)`. `list(path)` returns
sorted entry names. `current()` returns the process working directory and
`change(path)` changes it for the Nova process. `exists(path)` and `info(path)`
provide directory metadata. Calls are synchronous and follow host permissions.

## Paths

`get Paths` provides `join(path, ...)`, `normalize(path)`, `absolute(path)`,
`relative(path)`, `extension(path)`, `filename(path)`, `parent(path)`, and
`exists(path)`. Paths use Rust's host-platform path implementation; returned
paths are lossy UTF-8 strings when OS paths are not valid UTF-8. `relative`
returns a path relative to the current directory when possible and otherwise
returns an absolute path.

## System

`get System` exposes `os()`, `architecture()`, `username()`, `hostname()`,
`current_directory()`, `executable_path()`, `home_directory()`,
`temporary_directory()`, `environment(name)`, `set_environment(name, value)`,
and `available_parallelism()`. Missing environment values return `None`.
Environment changes affect only the current Nova process. Username, hostname,
and home-directory lookups can error when host information is unavailable.

## JSON

`get JSON` provides `parse(text)`, `stringify(value)`, `read(path)`, and
`write(path, value)`. JSON objects become dictionaries with string keys;
arrays become lists; `null` becomes `None`. Serialization rejects non-string
dictionary keys, non-finite floats, and runtime-only values such as UI handles
and functions. File operations are synchronous; `write` overwrites its path.

These modules use the portable Rust standard library plus the existing
`serde_json` dependency. They were exercised on Windows; the CI matrix is
intended to verify the portable core on Linux as well.

## Random

`Random.integer(min, max)` returns an inclusive integer; `float()` returns a
value in `[0, 1)`; `boolean()` returns a pseudorandom `Bool`; `choice(list)`
returns one element; `shuffle(list)` returns a shuffled copy; `seed(int)` sets
the deterministic pseudorandom generator seed. This generator is **not
cryptographically secure**. Use `Crypto.random_bytes` for secret material.

## Crypto

The `sha2` crate provides `Crypto.sha256(text)`, `Crypto.sha512(text)`, and
`Crypto.hmac_sha256(key, text)`, which return lowercase hexadecimal strings.
`Crypto.random_bytes(count)` returns operating-system secure random bytes as
`List[Int]`; the accepted count is 0 through 1,048,576. Hashes are one-way
digests, not encryption. Secure random uses the OS CSPRNG (`/dev/urandom` on
Unix-like systems and BCrypt on Windows).

## CSV

`CSV.parse(text)` and `CSV.read(path)` return rows as lists of strings.
`CSV.stringify(rows)` returns RFC-style escaped comma-separated text, and
`CSV.write(path, rows)` overwrites a file. Quoted fields, doubled quotes,
commas, and line breaks are handled. Rows must be lists; cell values use Nova's
string conversion.

## Logging

`Logging.info(message)`, `Logging.warning(message)`, and `Logging.error(message)`
write timestamped messages to stdout or stderr and return `None`. Logging is
synchronous and currently has no file rotation or structured secret redaction;
avoid passing secrets in messages.

## DateTime

`DateTime.now()` returns the current Unix timestamp in whole seconds,
`DateTime.today()` returns the current UTC date as `YYYY-MM-DD`,
`DateTime.parse(text)` accepts `YYYY-MM-DD` or UTC
`YYYY-MM-DDTHH:MM:SSZ` and returns Unix seconds, and
`DateTime.format(timestamp, pattern)` formats UTC values using `%Y`, `%m`,
`%d`, `%H`, `%M`, and `%S`. Time zones, fractional seconds, named formats, and
date arithmetic are not implemented yet.

## Process and Program discovery

`Process.list()` returns dictionaries containing process `name`, `pid`, and
`running`; `Process.find(name_or_pid)` returns all exact name matches or the
matching PID; `Process.info(name_or_pid)` requires exactly one match;
`Process.exists(name_or_pid)` and `Process.is_running(pid)` return booleans.
`Process.start(path)` or `Process.start(path, arguments)` launches a child
without a shell and returns its PID. Arguments must be a `List[String]`.
Listing uses PowerShell `Get-Process` on Windows and `ps` on Unix-like systems. Process
metadata is limited to what these commands provide. Child output capture,
waiting, graceful termination, and ownership handles are not implemented.

`Program.list/find/info/exists` currently provide process-based application
discovery only. They do not enumerate desktop windows, capture screens, or
identify a visible window by title. `Program.view` and the `Window` module are
not exposed until a consent-aware native window backend and capture lifecycle
are implemented. No arbitrary process memory or termination API is provided.

## HTTP and Network

`HTTP.get(url)` and `HTTP.post(url, text)` return dictionaries containing
`status`, UTF-8 `body`, `headers`, and final `url`. `HTTP.download(url, path)`
streams a successful response to a new destination and returns its byte count;
existing destinations are refused and incomplete temporary files are removed.
Requests use HTTPS certificate verification, HTTP/HTTPS schemes only, a
15-second request timeout, and at most five redirects. Text responses are
limited to 16 MiB; downloads are limited to 100 MiB with a 30-second timeout.
Calls are synchronous and cannot be cancelled while in progress. Binary body
access, custom headers, query builders, and progress callbacks are not yet
implemented. Requests to local addresses are permitted, so Nova programs
should only make requests their user intends.

`Network.resolve(host)` returns unique IP addresses from the operating system
resolver. It does not make an HTTP request.

## Image

`Image.load(path)` validates and decodes PNG, JPEG, WebP, or BMP and returns a
descriptor dictionary containing the source path and dimensions. The
descriptor can be passed to `width`, `height`, `dimensions`, and `pixel`.
`resize(image, destination, width, height)`,
`crop(image, destination, x, y, width, height)`,
`rotate(image, destination, degrees)`, `flip(image, destination, axis)`, and
`convert(image, destination, format)` write new images and return descriptors.
Outputs refuse to overwrite existing paths. Inputs are limited to 32 MiB,
dimensions to 8192 per side, decoded allocation to 256 MiB, and total pixels to
40 megapixels. Rotation supports 90, 180, or 270 degrees; flip axes are
`horizontal` and `vertical`. This is a synchronous file-based API; image data
is not yet a first-class Nova value or live UI stream.

## Archive

`Archive.create(path, files)` creates a ZIP archive from a `List[String]` of
file paths (entries use each source filename). `Archive.list(path)` returns
entry names. `Archive.extract(path, destination)` requires a new destination,
rejects unsafe archive paths, refuses existing extracted files, and enforces
100 MiB per entry and 500 MiB expanded total. It cleans up its destination if
extraction fails. Extraction supports ZIP only; broader archive formats and
directory creation are planned.
