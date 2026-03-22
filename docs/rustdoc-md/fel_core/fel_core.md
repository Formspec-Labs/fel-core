**fel_core**

# Module: fel_core

## Contents

**Modules**

- [`ast`](#ast) - FEL abstract syntax tree node definitions and operators.
- [`context_json`](#context_json) - Build [`FormspecEnvironment`] from JSON-shaped evaluation context.
- [`convert`](#convert) - Canonical conversion between serde_json::Value and FelValue.
- [`dependencies`](#dependencies) - Static dependency extraction — field refs, context refs, and MIP dependencies.
- [`environment`](#environment) - FEL evaluation environment with field resolution, repeats, MIP state, and instances.
- [`error`](#error) - FEL error types and diagnostic messages.
- [`evaluator`](#evaluator) - FEL tree-walking evaluator with base-10 decimal arithmetic and null propagation.
- [`extensions`](#extensions) - FEL extension function registry with null propagation and conflict detection.
- [`lexer`](#lexer) - FEL hand-rolled lexer — tokenization with spans and decimal numbers.
- [`parser`](#parser) - FEL hand-rolled recursive descent parser with operator precedence.
- [`printer`](#printer) - FEL AST to string serializer for expression rewriting and debugging.
- [`types`](#types) - FEL runtime value types with base-10 decimal arithmetic.
- [`wire_style`](#wire_style) - Host JSON key convention shared across FEL dependency wire and Formspec FFI surfaces.

**Structs**

- [`PositionedToken`](#positionedtoken) - One lexeme from [`tokenize`] for host bindings and tooling (stable type names + source span).

**Functions**

- [`eval_with_fields`](#eval_with_fields) - Parse and evaluate a FEL expression with a flat field map.
- [`fel_diagnostics_to_json_value`](#fel_diagnostics_to_json_value) - Evaluation diagnostics as JSON objects (`message`, `severity` wire string).
- [`tokenize`](#tokenize) - Tokenize FEL source into [`PositionedToken`]s (lexical analysis only; no parse).
- [`tokenize_to_json_value`](#tokenize_to_json_value) - FEL lexer tokens as JSON (camelCase keys) for host bindings.

---

## fel_core::PositionedToken

*Struct*

One lexeme from [`tokenize`] for host bindings and tooling (stable type names + source span).

**Fields:**
- `token_type: String` - Logical token kind (e.g. `NumberLiteral`, `Identifier`).
- `text: String` - Lexeme text from the source.
- `start: usize` - Start offset in Unicode scalar indices.
- `end: usize` - End offset (exclusive) in Unicode scalar indices.

**Traits:** Eq

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> PositionedToken`
- **PartialEq**
  - `fn eq(self: &Self, other: &PositionedToken) -> bool`



## Module: ast

FEL abstract syntax tree node definitions and operators.



## Module: context_json

Build [`FormspecEnvironment`] from JSON-shaped evaluation context.



## Module: convert

Canonical conversion between serde_json::Value and FelValue.

These are the single source of truth for JSON↔FEL value conversion.
All crates should use these instead of rolling their own.



## Module: dependencies

Static dependency extraction — field refs, context refs, and MIP dependencies.



## Module: environment

FEL evaluation environment with field resolution, repeats, MIP state, and instances.



## Module: error

FEL error types and diagnostic messages.



## fel_core::eval_with_fields

*Function*

Parse and evaluate a FEL expression with a flat field map.

```rust
fn eval_with_fields(input: &str, fields: std::collections::HashMap<String, FelValue>) -> Result<EvalResult, FelError>
```



## Module: evaluator

FEL tree-walking evaluator with base-10 decimal arithmetic and null propagation.



## Module: extensions

FEL extension function registry with null propagation and conflict detection.



## fel_core::fel_diagnostics_to_json_value

*Function*

Evaluation diagnostics as JSON objects (`message`, `severity` wire string).

```rust
fn fel_diagnostics_to_json_value(diagnostics: &[Diagnostic]) -> serde_json::Value
```



## Module: lexer

FEL hand-rolled lexer — tokenization with spans and decimal numbers.



## Module: parser

FEL hand-rolled recursive descent parser with operator precedence.



## Module: printer

FEL AST to string serializer for expression rewriting and debugging.



## fel_core::tokenize

*Function*

Tokenize FEL source into [`PositionedToken`]s (lexical analysis only; no parse).

```rust
fn tokenize(input: &str) -> Result<Vec<PositionedToken>, String>
```



## fel_core::tokenize_to_json_value

*Function*

FEL lexer tokens as JSON (camelCase keys) for host bindings.

```rust
fn tokenize_to_json_value(input: &str) -> Result<serde_json::Value, String>
```



## Module: types

FEL runtime value types with base-10 decimal arithmetic.



## Module: wire_style

Host JSON key convention shared across FEL dependency wire and Formspec FFI surfaces.



