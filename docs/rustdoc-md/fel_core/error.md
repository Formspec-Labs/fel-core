**fel_core > error**

# Module: error

## Contents

**Structs**

- [`Diagnostic`](#diagnostic) - A non-fatal diagnostic recorded during evaluation.

**Enums**

- [`FelError`](#felerror) - Failure from [`crate::parse`] or fatal-style evaluation errors surfaced as `Err`.
- [`Severity`](#severity) - Diagnostic severity for tooling and JSON wire format.

**Functions**

- [`reject_undefined_functions`](#reject_undefined_functions) - Returns `Err` when any undefined-function diagnostic is present (WASM / strict hosts).
- [`undefined_function_names_from_diagnostics`](#undefined_function_names_from_diagnostics) - Names from `undefined function: …` diagnostics (host bindings reject these as unsupported).

---

## fel_core::error::Diagnostic

*Struct*

A non-fatal diagnostic recorded during evaluation.

**Fields:**
- `severity: Severity` - Severity for hosts and JSON wire encoding.
- `message: String` - Human-readable explanation.

**Methods:**

- `fn error<impl Into<String>>(msg: impl Trait) -> Self` - Build an error-severity diagnostic.
- `fn warning<impl Into<String>>(msg: impl Trait) -> Self` - Build a warning-severity diagnostic.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Diagnostic`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::error::FelError

*Enum*

Failure from [`crate::parse`] or fatal-style evaluation errors surfaced as `Err`.

**Variants:**
- `Parse(String)` - Lex/parse failure (message from lexer or parser).
- `Eval(String)` - Evaluation failure where the API returns `Err` instead of diagnostics.

**Traits:** Error

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> FelError`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::error::Severity

*Enum*

Diagnostic severity for tooling and JSON wire format.

**Variants:**
- `Error` - Blocking / error-level.
- `Warning` - Warning-level.
- `Info` - Informational.

**Methods:**

- `fn as_wire_str(self: Self) -> &'static str` - Wire string used in JSON diagnostics (`error` / `warning` / `info`).

**Traits:** Eq, Copy

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &Severity) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Severity`



## fel_core::error::reject_undefined_functions

*Function*

Returns `Err` when any undefined-function diagnostic is present (WASM / strict hosts).

```rust
fn reject_undefined_functions(diagnostics: &[Diagnostic]) -> Result<(), String>
```



## fel_core::error::undefined_function_names_from_diagnostics

*Function*

Names from `undefined function: …` diagnostics (host bindings reject these as unsupported).

```rust
fn undefined_function_names_from_diagnostics(diagnostics: &[Diagnostic]) -> Vec<String>
```



