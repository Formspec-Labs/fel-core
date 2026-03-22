# fel-core — generated API (Markdown)

Generated: 2026-03-22T12:38:04.723Z (do not edit by hand; regenerate via npm script / cargo doc-md + this bundler)

Bundled from [cargo-doc-md](https://github.com/Crazytieguy/cargo-doc-md). Nested module paths are preserved in headings. Relative links may not resolve; search by heading.

---

## doc-md index

# Documentation Index

Generated markdown documentation for this project.

## Dependencies (1)

- [`fel-core`](fel_core/index.md)

---

Generated with [cargo-doc-md](https://github.com/Crazytieguy/cargo-doc-md)

---

## Source: fel_core/index.md

# fel_core

FEL parser, evaluator, and dependency analysis with base-10 decimal arithmetic.

Uses `rust_decimal` for base-10 arithmetic per spec S3.4.1 (minimum 18 significant digits).

## Docs

- Human overview: crate `README.md` (architecture, pipeline, module map).
- API reference: `cargo doc -p fel-core --no-deps --open`.
- Markdown API export: `docs/rustdoc-md/API.md` (see crate README).

## Modules

### [`fel_core`](fel_core.md)

*1 struct, 13 modules, 4 functions*

### [`ast`](ast.md)

*4 enums*

### [`context_json`](context_json.md)

*1 function*

### [`convert`](convert.md)

*4 functions*

### [`dependencies`](dependencies.md)

*1 struct, 3 functions*

### [`environment`](environment.md)

*3 structs*

### [`error`](error.md)

*1 struct, 2 enums, 2 functions*

### [`evaluator`](evaluator.md)

*1 function, 1 trait, 3 structs*

### [`extensions`](extensions.md)

*1 enum, 1 type alias, 2 functions, 3 structs*

### [`lexer`](lexer.md)

*1 enum, 3 structs*

### [`parser`](parser.md)

*1 function, 1 struct*

### [`printer`](printer.md)

*1 function*

### [`types`](types.md)

*1 struct, 2 enums, 6 functions*

### [`wire_style`](wire_style.md)

*1 enum*

---

## Source: fel_core/fel_core.md

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

`push_repeat_context` recursively walks nested repeat JSON into environment state.



## Module: convert

Canonical conversion between serde_json::Value and FelValue.

These are the single source of truth for JSON↔FEL value conversion.
All crates should use these instead of rolling their own.



## Module: dependencies

Static dependency extraction — field refs, context refs, and MIP dependencies.

Walks the AST without evaluation to find field references,
context references, MIP dependencies, and structural flags.

The `walk` helper and related functions recurse the AST to populate [`Dependencies`].



## Module: environment

FEL evaluation environment with field resolution, repeats, MIP state, and instances.

Provides `FormspecEnvironment`, a concrete `Environment` impl backed by
nested data dicts, repeat context, MIP states, named instances, and variables.

Helpers such as `project_repeat_field` resolve repeat-group keys into projected field values.



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

Non-fatal errors produce a Diagnostic + FelNull (never panic).
Null propagation follows spec §3: most ops propagate, equality does NOT.

The [`Evaluator`] owns `let` scopes and builtins; private `eval` / `fn_*` methods implement the tree walk.



## Module: extensions

FEL extension function registry with null propagation and conflict detection.

Extensions cannot shadow reserved words or built-in function names.
All extension functions are null-propagating: if any argument is null, the result is null.

Registration, dispatch, and `BUILTIN_FUNCTIONS` back the catalog / WASM surfaces.



## fel_core::fel_diagnostics_to_json_value

*Function*

Evaluation diagnostics as JSON objects (`message`, `severity` wire string).

```rust
fn fel_diagnostics_to_json_value(diagnostics: &[Diagnostic]) -> serde_json::Value
```



## Module: lexer

FEL hand-rolled lexer — tokenization with spans and decimal numbers.

Internal scanning uses a char buffer and cursor; [`Lexer::tokenize`] is the public entry point.



## Module: parser

FEL hand-rolled recursive descent parser with operator precedence.

Private `parse_*` / `current` / `advance` implement the precedence ladder listed below.



## Module: printer

FEL AST to string serializer for expression rewriting and debugging.

Used by the assembler to rewrite FEL expressions after AST transformations
(e.g., field path prefixing during $ref resolution).

`write_expr` and helpers serialize each [`Expr`] variant; parentheses only when needed.



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

---

## Source: fel_core/ast.md

**fel_core > ast**

# Module: ast

## Contents

**Enums**

- [`BinaryOp`](#binaryop) - Binary and logical operators (precedence enforced in the parser).
- [`Expr`](#expr) - Expression AST for Formspec Expression Language (FEL).
- [`PathSegment`](#pathsegment) - A path segment for field references and postfix access (`$a.b`, `$a[1]`, `$a[*]`).
- [`UnaryOp`](#unaryop) - Unary operators (`not`, unary `-`).

---

## fel_core::ast::BinaryOp

*Enum*

Binary and logical operators (precedence enforced in the parser).

**Variants:**
- `Add` - `+`
- `Sub` - `-`
- `Mul` - `*`
- `Div` - `/`
- `Mod` - `%`
- `Concat` - `&` string concatenation.
- `Eq` - `=` or `==`
- `NotEq` - `!=`
- `Lt` - `<`
- `Gt` - `>`
- `LtEq` - `<=`
- `GtEq` - `>=`
- `And` - `and`
- `Or` - `or`

**Traits:** Eq, Copy

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &BinaryOp) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> BinaryOp`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::ast::Expr

*Enum*

Expression AST for Formspec Expression Language (FEL).

Covers literals, operators, `let`/`if`, function calls, `$` field refs, and `@` context refs.
Shape follows `specs/fel/fel-grammar.llm.md` in the Formspec repo.

**Variants:**
- `Null`
- `Boolean(bool)`
- `Number(rust_decimal::Decimal)`
- `String(String)`
- `DateLiteral(String)`
- `DateTimeLiteral(String)`
- `Array(Vec<Expr>)`
- `Object(Vec<(String, Expr)>)`
- `FieldRef{ name: Option<String>, path: Vec<PathSegment> }`
- `ContextRef{ name: String, arg: Option<String>, tail: Vec<String> }`
- `UnaryOp{ op: UnaryOp, operand: Box<Expr> }`
- `BinaryOp{ op: BinaryOp, left: Box<Expr>, right: Box<Expr> }`
- `Ternary{ condition: Box<Expr>, then_branch: Box<Expr>, else_branch: Box<Expr> }`
- `IfThenElse{ condition: Box<Expr>, then_branch: Box<Expr>, else_branch: Box<Expr> }`
- `Membership{ value: Box<Expr>, container: Box<Expr>, negated: bool }`
- `NullCoalesce{ left: Box<Expr>, right: Box<Expr> }`
- `LetBinding{ name: String, value: Box<Expr>, body: Box<Expr> }`
- `FunctionCall{ name: String, args: Vec<Expr> }`
- `PostfixAccess{ expr: Box<Expr>, path: Vec<PathSegment> }`

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Expr`
- **PartialEq**
  - `fn eq(self: &Self, other: &Expr) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::ast::PathSegment

*Enum*

A path segment for field references and postfix access (`$a.b`, `$a[1]`, `$a[*]`).

**Variants:**
- `Dot(String)` - Property after a dot (identifier name).
- `Index(usize)` - Numeric index inside `[` `]`.
- `Wildcard` - Repeat wildcard `[*]`.

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &PathSegment) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> PathSegment`



## fel_core::ast::UnaryOp

*Enum*

Unary operators (`not`, unary `-`).

**Variants:**
- `Not` - Logical not.
- `Neg` - Arithmetic negation.

**Traits:** Eq, Copy

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &UnaryOp) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> UnaryOp`

---

## Source: fel_core/context_json.md

**fel_core > context_json**

# Module: context_json

## Contents

**Functions**

- [`formspec_environment_from_json_map`](#formspec_environment_from_json_map) - Populate a [`FormspecEnvironment`] from a JSON object (e.g. WASM `evalFELWithContext` payload).

---

## fel_core::context_json::formspec_environment_from_json_map

*Function*

Populate a [`FormspecEnvironment`] from a JSON object (e.g. WASM `evalFELWithContext` payload).

Recognized keys: `nowIso` / `now_iso`, `fields`, `variables`, `mipStates` / `mip_states`,
`repeatContext` / `repeat_context`, `instances`.

```rust
fn formspec_environment_from_json_map(ctx: &serde_json::Map<String, serde_json::Value>) -> crate::FormspecEnvironment
```

---

## Source: fel_core/convert.md

**fel_core > convert**

# Module: convert

## Contents

**Functions**

- [`fel_to_json`](#fel_to_json) - Convert a `FelValue` to a `serde_json::Value`.
- [`field_map_from_json_str`](#field_map_from_json_str) - Parse a JSON object string into a field map (empty or `"{}"` → empty map).
- [`json_object_to_field_map`](#json_object_to_field_map) - JSON object → flat field map for FEL `MapEnvironment` (`{}` / empty → empty map).
- [`json_to_fel`](#json_to_fel) - Convert a `serde_json::Value` to a `FelValue`.

---

## fel_core::convert::fel_to_json

*Function*

Convert a `FelValue` to a `serde_json::Value`.

Conversion rules:
- `Null` → `Value::Null`
- `Boolean(b)` → `Value::Bool(b)`
- `Number(n)` → `Value::Number` (integer when whole, f64 otherwise)
- `String(s)` → `Value::String(s)`
- `Date(d)` → `Value::String(d.format_iso())`
- `Money { amount, currency }` → `{"$type": "money", "amount": <number>, "currency": <string>}`
- `Array(arr)` → `Value::Array` (recursive)
- `Object(entries)` → `Value::Object` (recursive)

```rust
fn fel_to_json(val: &crate::types::FelValue) -> serde_json::Value
```



## fel_core::convert::field_map_from_json_str

*Function*

Parse a JSON object string into a field map (empty or `"{}"` → empty map).

```rust
fn field_map_from_json_str(fields_json: &str) -> Result<std::collections::HashMap<String, crate::types::FelValue>, String>
```



## fel_core::convert::json_object_to_field_map

*Function*

JSON object → flat field map for FEL `MapEnvironment` (`{}` / empty → empty map).

```rust
fn json_object_to_field_map(val: &serde_json::Value) -> std::collections::HashMap<String, crate::types::FelValue>
```



## fel_core::convert::json_to_fel

*Function*

Convert a `serde_json::Value` to a `FelValue`.

Conversion rules:
- `Null` → `FelValue::Null`
- `Bool(b)` → `FelValue::Boolean(b)`
- `Number(n)` → `FelValue::Number` (tries i64, then u64, then f64)
- `String(s)` → `FelValue::String(s)` — no silent date coercion
- `Array(arr)` → `FelValue::Array` (recursive)
- `Object` with `"$type": "money"` + `"amount"` + `"currency"` → `FelValue::Money`
- `Object` otherwise → `FelValue::Object` (recursive)

Money detection requires an explicit `"$type": "money"` marker. Objects that
happen to have `amount` and `currency` fields but lack the marker are treated
as regular objects — no heuristic guessing.

The `amount` field accepts either a JSON number or a JSON string that parses
as a Decimal.

```rust
fn json_to_fel(val: &serde_json::Value) -> crate::types::FelValue
```

---

## Source: fel_core/dependencies.md

**fel_core > dependencies**

# Module: dependencies

## Contents

**Structs**

- [`Dependencies`](#dependencies) - Dependencies extracted from a FEL expression.

**Functions**

- [`dependencies_to_json_value`](#dependencies_to_json_value) - Serialize [`Dependencies`] for WASM / JSON FFI (camelCase keys).
- [`dependencies_to_json_value_styled`](#dependencies_to_json_value_styled) - Serialize [`Dependencies`] with explicit host key style.
- [`extract_dependencies`](#extract_dependencies) - Extract dependencies from an AST expression.

---

## fel_core::dependencies::Dependencies

*Struct*

Dependencies extracted from a FEL expression.

**Fields:**
- `fields: std::collections::HashSet<String>` - Field paths referenced (e.g., `["firstName", "address.city"]`).
- `context_refs: std::collections::HashSet<String>` - Context references (e.g., `["@current", "@index"]`).
- `instance_refs: std::collections::HashSet<String>` - Instance references from `@instance('name')`.
- `mip_deps: std::collections::HashSet<String>` - MIP dependencies: fields used in valid/relevant/readonly/required.
- `has_self_ref: bool` - Whether bare `$` (self-reference) appears.
- `has_wildcard: bool` - Whether any `[*]` wildcard appears.
- `uses_prev_next: bool` - Whether prev() or next() is called.

**Trait Implementations:**

- **Default**
  - `fn default() -> Dependencies`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> Dependencies`



## fel_core::dependencies::dependencies_to_json_value

*Function*

Serialize [`Dependencies`] for WASM / JSON FFI (camelCase keys).

```rust
fn dependencies_to_json_value(deps: &Dependencies) -> serde_json::Value
```



## fel_core::dependencies::dependencies_to_json_value_styled

*Function*

Serialize [`Dependencies`] with explicit host key style.

```rust
fn dependencies_to_json_value_styled(deps: &Dependencies, style: crate::wire_style::JsonWireStyle) -> serde_json::Value
```



## fel_core::dependencies::extract_dependencies

*Function*

Extract dependencies from an AST expression.

```rust
fn extract_dependencies(expr: &Expr) -> Dependencies
```

---

## Source: fel_core/environment.md

**fel_core > environment**

# Module: environment

## Contents

**Structs**

- [`FormspecEnvironment`](#formspecenvironment) - A full-featured environment for FEL evaluation within a Formspec engine.
- [`MipState`](#mipstate) - XForms Model Item Properties for a single field path.
- [`RepeatContext`](#repeatcontext) - Repeat-group iteration context (§4.3).

---

## fel_core::environment::FormspecEnvironment

*Struct*

A full-featured environment for FEL evaluation within a Formspec engine.

Supports:
- Field resolution via `$field.path` (walks nested data dict)
- Named instances via `@instance('name')`
- Repeat context via `@current`, `@index`, `@count`
- MIP state queries via `valid()`, `relevant()`, etc.
- Definition variables via `@variableName`
- Mapping context via `@source`, `@target`

**Fields:**
- `data: std::collections::HashMap<String, crate::types::FelValue>` - Primary data dict — backs `$field` references.
- `instances: std::collections::HashMap<String, crate::types::FelValue>` - Named secondary instances — backs `@instance('name')`.
- `mip_states: std::collections::HashMap<String, MipState>` - MIP states per dotted field path.
- `variables: std::collections::HashMap<String, crate::types::FelValue>` - Definition variables — backs `@variableName`.
- `repeat_context: Option<RepeatContext>` - Current repeat context (if inside a repeat iteration).
- `current_datetime: Option<crate::types::FelDate>` - Current runtime date for today()/now().

**Methods:**

- `fn new() -> Self` - Empty environment (no data, instances, or repeat context).
- `fn set_field(self: & mut Self, path: &str, value: FelValue)` - Set a field value by dotted path (e.g., "address.city").
- `fn set_instance(self: & mut Self, name: &str, value: FelValue)` - Set a named instance.
- `fn set_mip(self: & mut Self, path: &str, state: MipState)` - Set MIP state for a field path.
- `fn set_variable(self: & mut Self, name: &str, value: FelValue)` - Set a variable value.
- `fn set_now_from_iso(self: & mut Self, iso: &str)` - Set the current runtime datetime from an ISO string.
- `fn push_repeat(self: & mut Self, current: FelValue, index: usize, count: usize, collection: Vec<FelValue>)` - Enter a repeat context.
- `fn pop_repeat(self: & mut Self)` - Leave the current repeat context, restoring the parent.

**Trait Implementations:**

- **Environment**
  - `fn resolve_field(self: &Self, segments: &[String]) -> FelValue`
  - `fn resolve_context(self: &Self, name: &str, arg: Option<&str>, tail: &[String]) -> FelValue`
  - `fn mip_valid(self: &Self, path: &[String]) -> FelValue`
  - `fn mip_relevant(self: &Self, path: &[String]) -> FelValue`
  - `fn mip_readonly(self: &Self, path: &[String]) -> FelValue`
  - `fn mip_required(self: &Self, path: &[String]) -> FelValue`
  - `fn repeat_prev(self: &Self) -> FelValue`
  - `fn repeat_next(self: &Self) -> FelValue`
  - `fn repeat_parent(self: &Self) -> FelValue`
  - `fn current_date(self: &Self) -> Option<FelDate>`
  - `fn current_datetime(self: &Self) -> Option<FelDate>`
- **Default**
  - `fn default() -> Self`



## fel_core::environment::MipState

*Struct*

XForms Model Item Properties for a single field path.

**Fields:**
- `valid: bool` - `valid($path)` result when set for this path.
- `relevant: bool` - `relevant($path)`.
- `readonly: bool` - `readonly($path)`.
- `required: bool` - `required($path)`.

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> MipState`



## fel_core::environment::RepeatContext

*Struct*

Repeat-group iteration context (§4.3).

**Fields:**
- `current: crate::types::FelValue` - The current row value.
- `index: usize` - 1-based index within the repeat group.
- `count: usize` - Total instance count.
- `parent: Option<Box<RepeatContext>>` - Parent repeat context (for nested repeats).
- `collection: Vec<crate::types::FelValue>` - All rows in the collection (for prev/next navigation).

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> RepeatContext`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`

---

## Source: fel_core/error.md

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

---

## Source: fel_core/evaluator.md

**fel_core > evaluator**

# Module: evaluator

## Contents

**Structs**

- [`EvalResult`](#evalresult) - Result of evaluation: a value plus any accumulated diagnostics.
- [`Evaluator`](#evaluator) - Tree-walking evaluator with `let` scopes and diagnostic collection.
- [`MapEnvironment`](#mapenvironment) - Flat `HashMap` environment for tests and simple hosts (no `@` context; fixed clock in default impl).

**Functions**

- [`evaluate`](#evaluate) - Evaluate an expression against an environment.

**Traits**

- [`Environment`](#environment) - Resolves `$` field paths, `@` context, MIP queries, repeat navigation, and clock for FEL builtins.

---

## fel_core::evaluator::Environment

*Trait*

Resolves `$` field paths, `@` context, MIP queries, repeat navigation, and clock for FEL builtins.

**Methods:**

- `resolve_field`: Resolve `$a.b` style path as segment list (`["a","b"]`); empty slice is bare `$`.
- `resolve_context`: Resolve `@name`, `@name('arg')`, `@name.tail`.
- `mip_valid`: `valid($path)` — default `true` when not overridden.
- `mip_relevant`: `relevant($path)` — default `true`.
- `mip_readonly`: `readonly($path)` — default `false`.
- `mip_required`: `required($path)` — default `false`.
- `repeat_prev`: `prev()` in repeat scope — default null.
- `repeat_next`: `next()` in repeat scope — default null.
- `repeat_parent`: `parent()` in repeat scope — default null.
- `current_date`: Calendar date for `today()` — default none (evaluator may still use literals).
- `current_datetime`: Date-time for `now()` — default none.



## fel_core::evaluator::EvalResult

*Struct*

Result of evaluation: a value plus any accumulated diagnostics.

**Fields:**
- `value: FelValue` - Computed value (may be null after errors).
- `diagnostics: Vec<crate::error::Diagnostic>` - Non-fatal issues (undefined functions, type errors, etc.).

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> EvalResult`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::evaluator::Evaluator

*Struct*

Tree-walking evaluator with `let` scopes and diagnostic collection.

**Generic Parameters:**
- 'a



## fel_core::evaluator::MapEnvironment

*Struct*

Flat `HashMap` environment for tests and simple hosts (no `@` context; fixed clock in default impl).

**Fields:**
- `fields: std::collections::HashMap<String, FelValue>` - Top-level and nested values (nested via object values); keys may be dotted.

**Methods:**

- `fn new() -> Self` - Empty field map.
- `fn with_fields(fields: HashMap<String, FelValue>) -> Self` - Pre-populated field map.

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`
- **Environment**
  - `fn resolve_field(self: &Self, segments: &[String]) -> FelValue`
  - `fn resolve_context(self: &Self, _name: &str, _arg: Option<&str>, _tail: &[String]) -> FelValue`
  - `fn current_date(self: &Self) -> Option<FelDate>`
  - `fn current_datetime(self: &Self) -> Option<FelDate>`



## fel_core::evaluator::evaluate

*Function*

Evaluate an expression against an environment.

```rust
fn evaluate(expr: &Expr, env: &dyn Environment) -> EvalResult
```

---

## Source: fel_core/extensions.md

**fel_core > extensions**

# Module: extensions

## Contents

**Structs**

- [`BuiltinFunctionCatalogEntry`](#builtinfunctioncatalogentry) - Metadata for a built-in FEL function exposed to tooling surfaces (WASM catalog, docs).
- [`ExtensionFunc`](#extensionfunc) - A registered extension function.
- [`ExtensionRegistry`](#extensionregistry) - Registry of extension functions.

**Enums**

- [`ExtensionError`](#extensionerror) - Error type for extension registration failures.

**Functions**

- [`builtin_function_catalog`](#builtin_function_catalog) - Slice of all built-in functions (names reserved for [`ExtensionRegistry::register`]).
- [`builtin_function_catalog_json_value`](#builtin_function_catalog_json_value) - Built-in catalog as a JSON array for WASM / tooling.

**Type Aliases**

- [`ExtensionFn`](#extensionfn) - Type alias for extension function implementations.

---

## fel_core::extensions::BuiltinFunctionCatalogEntry

*Struct*

Metadata for a built-in FEL function exposed to tooling surfaces (WASM catalog, docs).

**Fields:**
- `name: &'static str` - Function name as in FEL source.
- `category: &'static str` - Grouping (e.g. `aggregate`, `string`, `repeat`).
- `signature: &'static str` - Human-readable arity and types.
- `description: &'static str` - Short description for UI or generated docs.



## fel_core::extensions::ExtensionError

*Enum*

Error type for extension registration failures.

**Variants:**
- `NameConflict(String)` - Registration rejected: name matches a reserved word or built-in function.

**Traits:** Error

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ExtensionError`
- **Display**
  - `fn fmt(self: &Self, f: & mut std::fmt::Formatter) -> std::fmt::Result`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::extensions::ExtensionFn

*Type Alias*: `Box<dyn Fn>`

Type alias for extension function implementations.



## fel_core::extensions::ExtensionFunc

*Struct*

A registered extension function.

**Fields:**
- `name: String` - Human-readable name for diagnostics.
- `min_args: usize` - Minimum number of arguments.
- `max_args: Option<usize>` - Maximum number of arguments (None = unbounded).
- `func: ExtensionFn` - The implementation: receives pre-evaluated args, returns a value.



## fel_core::extensions::ExtensionRegistry

*Struct*

Registry of extension functions.

**Methods:**

- `fn new() -> Self` - Empty registry (no custom extensions).
- `fn register<impl Into<String>, impl Fn(&[FelValue]) -> FelValue + Send + Sync + 'static>(self: & mut Self, name: impl Trait, min_args: usize, max_args: Option<usize>, func: impl Trait) -> Result<(), ExtensionError>` - Register an extension function.
- `fn get(self: &Self, name: &str) -> Option<&ExtensionFunc>` - Look up an extension function by name.
- `fn contains(self: &Self, name: &str) -> bool` - Check if a name is registered.
- `fn call(self: &Self, name: &str, args: &[FelValue]) -> Option<FelValue>` - Call an extension function with null propagation.

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`



## fel_core::extensions::builtin_function_catalog

*Function*

Slice of all built-in functions (names reserved for [`ExtensionRegistry::register`]).

```rust
fn builtin_function_catalog() -> &'static [BuiltinFunctionCatalogEntry]
```



## fel_core::extensions::builtin_function_catalog_json_value

*Function*

Built-in catalog as a JSON array for WASM / tooling.

```rust
fn builtin_function_catalog_json_value() -> serde_json::Value
```

---

## Source: fel_core/lexer.md

**fel_core > lexer**

# Module: lexer

## Contents

**Structs**

- [`Lexer`](#lexer) - Character-based lexer over a FEL expression string.
- [`Span`](#span) - Byte/char span in the original source (Unicode scalar indices, inclusive start, exclusive end).
- [`SpannedToken`](#spannedtoken) - A [`Token`] with its [`Span`].

**Enums**

- [`Token`](#token) - Lexical token for FEL source (literals, keywords, operators, punctuation).

---

## fel_core::lexer::Lexer

*Struct*

Character-based lexer over a FEL expression string.

**Generic Parameters:**
- 'a

**Methods:**

- `fn new(input: &'a str) -> Self` - Create a lexer for `input` (no allocation beyond char buffer).
- `fn tokenize(self: & mut Self) -> Result<Vec<SpannedToken>, String>` - Consume the entire input and return all tokens, ending with [`Token::Eof`].



## fel_core::lexer::Span

*Struct*

Byte/char span in the original source (Unicode scalar indices, inclusive start, exclusive end).

**Fields:**
- `start: usize` - Start offset.
- `end: usize` - End offset (exclusive).

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> Span`



## fel_core::lexer::SpannedToken

*Struct*

A [`Token`] with its [`Span`].

**Fields:**
- `token: Token` - Classified token.
- `span: Span` - Position in source.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> SpannedToken`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::lexer::Token

*Enum*

Lexical token for FEL source (literals, keywords, operators, punctuation).

**Variants:**
- `Number(rust_decimal::Decimal)`
- `StringLit(String)`
- `True`
- `False`
- `Null`
- `DateLiteral(String)`
- `DateTimeLiteral(String)`
- `Identifier(String)`
- `Let`
- `In`
- `If`
- `Then`
- `Else`
- `And`
- `Or`
- `Not`
- `Plus`
- `Minus`
- `Star`
- `Slash`
- `Percent`
- `Ampersand`
- `Eq`
- `NotEq`
- `Lt`
- `Gt`
- `LtEq`
- `GtEq`
- `DoubleQuestion`
- `Question`
- `LParen`
- `RParen`
- `LBracket`
- `RBracket`
- `LBrace`
- `RBrace`
- `Comma`
- `Dot`
- `Colon`
- `Dollar`
- `At`
- `Eof`

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> Token`
- **PartialEq**
  - `fn eq(self: &Self, other: &Token) -> bool`

---

## Source: fel_core/parser.md

**fel_core > parser**

# Module: parser

## Contents

**Structs**

- [`Parser`](#parser) - Recursive-descent parser over a [`SpannedToken`] stream (use [`parse`] to build from source).

**Functions**

- [`parse`](#parse) - Parse a FEL expression string into an AST.

---

## fel_core::parser::Parser

*Struct*

Recursive-descent parser over a [`SpannedToken`] stream (use [`parse`] to build from source).



## fel_core::parser::parse

*Function*

Parse a FEL expression string into an AST.

```rust
fn parse(input: &str) -> Result<Expr, crate::error::FelError>
```

---

## Source: fel_core/printer.md

**fel_core > printer**

# Module: printer

## Contents

**Functions**

- [`print_expr`](#print_expr) - Print a FEL expression AST back to a source string.

---

## fel_core::printer::print_expr

*Function*

Print a FEL expression AST back to a source string.

```rust
fn print_expr(expr: &Expr) -> String
```

---

## Source: fel_core/types.md

**fel_core > types**

# Module: types

## Contents

**Structs**

- [`FelMoney`](#felmoney) - Monetary value with ISO currency code.

**Enums**

- [`FelDate`](#feldate) - Calendar date or date-time (no timezone model; used by date functions).
- [`FelValue`](#felvalue) - Runtime value for FEL evaluation (mirrors JSON + dates + money).

**Functions**

- [`civil_from_days_pub`](#civil_from_days_pub) - Convert days since the internal FEL epoch (2000-01-01) to a [`FelDate`] (date-only).
- [`date_add_days`](#date_add_days) - Add days to a date.
- [`days_in_month`](#days_in_month) - Gregorian days in `month` for `year` (validates `month` in debug only).
- [`format_number`](#format_number) - Format a Decimal: strip trailing zeros, show as integer when possible.
- [`parse_date_literal`](#parse_date_literal) - Parse "@YYYY-MM-DD" into FelDate.
- [`parse_datetime_literal`](#parse_datetime_literal) - Parse "@YYYY-MM-DDTHH:MM:SS..." into FelDate.

---

## fel_core::types::FelDate

*Enum*

Calendar date or date-time (no timezone model; used by date functions).

**Variants:**
- `Date{ year: i32, month: u32, day: u32 }`
- `DateTime{ year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32 }`

**Methods:**

- `fn year(self: &Self) -> i32` - Calendar year component.
- `fn month(self: &Self) -> u32` - Month 1–12.
- `fn day(self: &Self) -> u32` - Day of month.
- `fn to_naive_date(self: &Self) -> (i32, u32, u32)` - `(year, month, day)` tuple.
- `fn ordinal_days(self: &Self) -> i64` - Days since epoch (2000-01-01) for ordering.
- `fn ordinal(self: &Self) -> i64` - Full ordinal including time (seconds from epoch) for DateTime ordering.
- `fn format_iso(self: &Self) -> String` - `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS` (no timezone suffix).

**Traits:** Eq

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> FelDate`
- **PartialEq**
  - `fn eq(self: &Self, other: &FelDate) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::types::FelMoney

*Struct*

Monetary value with ISO currency code.

**Fields:**
- `amount: rust_decimal::Decimal` - Decimal amount (base-10).
- `currency: String` - ISO 4217 currency code (e.g. `USD`).

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Self) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> FelMoney`



## fel_core::types::FelValue

*Enum*

Runtime value for FEL evaluation (mirrors JSON + dates + money).

**Variants:**
- `Null`
- `Boolean(bool)`
- `Number(rust_decimal::Decimal)`
- `String(String)`
- `Date(FelDate)`
- `Array(Vec<FelValue>)`
- `Object(Vec<(String, FelValue)>)`
- `Money(FelMoney)`

**Methods:**

- `fn type_name(self: &Self) -> &'static str` - Lowercase FEL type name for error messages.
- `fn is_null(self: &Self) -> bool` - True only for [`FelValue::Null`].
- `fn is_truthy(self: &Self) -> bool` - Loose truth test (not FEL `and`/`or` typing — used by some builtins).
- `fn as_number(self: &Self) -> Option<Decimal>` - Extract number or `None`.
- `fn as_string(self: &Self) -> Option<&str>` - Borrow string or `None`.
- `fn as_bool(self: &Self) -> Option<bool>` - Extract boolean or `None`.
- `fn as_date(self: &Self) -> Option<&FelDate>` - Borrow date/datetime or `None`.
- `fn as_array(self: &Self) -> Option<&Vec<FelValue>>` - Borrow array or `None`.
- `fn as_money(self: &Self) -> Option<&FelMoney>` - Borrow money or `None`.

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Self) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> FelValue`



## fel_core::types::civil_from_days_pub

*Function*

Convert days since the internal FEL epoch (2000-01-01) to a [`FelDate`] (date-only).

```rust
fn civil_from_days_pub(z: i64) -> FelDate
```



## fel_core::types::date_add_days

*Function*

Add days to a date.

```rust
fn date_add_days(d: &FelDate, n: i64) -> FelDate
```



## fel_core::types::days_in_month

*Function*

Gregorian days in `month` for `year` (validates `month` in debug only).

```rust
fn days_in_month(year: i32, month: u32) -> u32
```



## fel_core::types::format_number

*Function*

Format a Decimal: strip trailing zeros, show as integer when possible.

```rust
fn format_number(n: rust_decimal::Decimal) -> String
```



## fel_core::types::parse_date_literal

*Function*

Parse "@YYYY-MM-DD" into FelDate.

```rust
fn parse_date_literal(s: &str) -> Option<FelDate>
```



## fel_core::types::parse_datetime_literal

*Function*

Parse "@YYYY-MM-DDTHH:MM:SS..." into FelDate.

```rust
fn parse_datetime_literal(s: &str) -> Option<FelDate>
```

---

## Source: fel_core/wire_style.md

**fel_core > wire_style**

# Module: wire_style

## Contents

**Enums**

- [`JsonWireStyle`](#jsonwirestyle) - JSON object key style for WASM (`camelCase`) vs Python (`snake_case`) bindings.

---

## fel_core::wire_style::JsonWireStyle

*Enum*

JSON object key style for WASM (`camelCase`) vs Python (`snake_case`) bindings.

**Variants:**
- `JsCamel` - JavaScript / `wasm-bindgen` (camelCase keys).
- `PythonSnake` - Python `formspec_rust` surface (snake_case keys).

**Traits:** Eq, Copy

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &JsonWireStyle) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> JsonWireStyle`

---

