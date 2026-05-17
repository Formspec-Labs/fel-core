# fel-core - generated API (Markdown)

> Do not edit by hand; regenerate with `npm run docs:fel-core`.

Bundled from cargo-doc-md output. Nested module paths are preserved in headings.

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
- API reference: `cargo doc --no-deps --open`.
- Markdown API export: `docs/rustdoc-md/API.md` (see crate README).

## Modules

### [`fel_core`](fel_core.md)

*12 modules*

### [`ast`](ast.md)

*4 enums*

### [`context_json`](context_json.md)

*1 function*

### [`convert`](convert.md)

*6 functions*

### [`dependencies`](dependencies.md)

*1 struct, 3 functions*

### [`environment`](environment.md)

*3 structs*

### [`error`](error.md)

*2 structs, 3 enums, 5 functions*

### [`evaluator`](evaluator.md)

*1 function*

### [`evaluator::budget`](evaluator/budget.md)

*1 enum, 1 struct*

### [`evaluator::core`](evaluator/core.md)

*1 trait, 2 functions, 4 structs*

### [`extensions::catalog`](extensions/catalog.md)

*2 functions*

### [`extensions::registry`](extensions/registry.md)

*1 enum, 1 struct*

### [`extensions::schema`](extensions/schema.md)

*3 functions*

### [`extensions::types`](extensions/types.md)

*1 type alias, 2 enums, 4 structs*

### [`interpolation`](interpolation.md)

*1 function*

### [`iso_duration`](iso_duration.md)

*1 enum, 2 functions*

### [`lexer`](lexer.md)

*1 enum, 4 structs, 5 functions*

### [`parser`](parser.md)

*1 function, 1 struct*

### [`prepare_host`](prepare_host.md)

*2 structs, 3 functions*

### [`printer`](printer.md)

*1 function*

### [`trace`](trace.md)

*1 enum, 1 struct*

### [`types`](types.md)

*2 enums, 2 structs, 5 functions*

### [`wire_style`](wire_style.md)

*1 enum*

---

## Source: fel_core/fel_core.md

**fel_core**

# Module: fel_core

## Contents

**Modules**

- [`ast`](#ast) - FEL abstract syntax tree node definitions and operators.
- [`convert`](#convert) - Canonical conversion between serde_json::Value and TypeValue.
- [`dependencies`](#dependencies) - Static dependency extraction — field refs, context refs, and MIP dependencies.
- [`environment`](#environment) - FEL evaluation environment with field resolution, repeats, MIP state, and instances.
- [`error`](#error) - FEL error types and diagnostic messages.
- [`evaluator`](#evaluator) - FEL tree-walking evaluator with base-10 decimal arithmetic and null propagation.
- [`extensions`](#extensions) - FEL extension function registry with null propagation and conflict detection.
- [`lexer`](#lexer) - FEL hand-rolled lexer — tokenization with spans and decimal numbers.
- [`parser`](#parser) - FEL hand-rolled recursive descent parser with operator precedence.
- [`prepare_host`](#prepare_host) - FEL source normalization before host evaluation (parity with TS `normalizeExpressionForWasmEvaluation`).
- [`printer`](#printer) - FEL AST to string serializer for expression rewriting and debugging.
- [`types`](#types) - FEL runtime value types with base-10 decimal arithmetic.

---

## Module: ast

FEL abstract syntax tree node definitions and operators.



## Module: convert

Canonical conversion between serde_json::Value and TypeValue.

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

## Design note (spec: core/spec.md §3.12, registry/extension-registry.md §7)

`ExtensionRegistry` is intentionally isolated from the evaluator's built-in
function dispatch. The spec says extensions "MAY supplement but MUST NOT
override" built-ins. This is enforced structurally: the evaluator matches
built-in names first in `eval_function`, and only falls through to the
extension registry for unknown names. The registry itself independently
rejects registration of names that collide with built-ins or reserved words.

This two-layer defense is by design, not accident. The evaluator's match
arms guarantee built-in semantics can never be replaced at runtime, while
the registry's registration-time check gives early feedback to extension
authors. Neither layer alone would be sufficient: without the evaluator
guard, a bug in the registry could allow shadowing; without the registry
guard, extensions would silently be ignored instead of rejected.



## Module: lexer

FEL hand-rolled lexer — tokenization with spans and decimal numbers.

Internal scanning uses a char buffer and cursor; [`Lexer::tokenize`] is the public entry point.



## Module: parser

FEL hand-rolled recursive descent parser with operator precedence.

Chaining multiple `==` / `!=` or multiple comparison operators is a **parse error**; write
explicit conjunction (e.g. `0 <= $x and $x <= 10`).

Private `parse_*` / `current` / `advance` implement the precedence ladder listed below.



## Module: prepare_host

FEL source normalization before host evaluation (parity with TS `normalizeExpressionForWasmEvaluation`).

Rewrites bare `$`, qualified repeat group refs (`$group.field`), and repeat row aliases into wildcard paths.



## Module: printer

FEL AST to string serializer for expression rewriting and debugging.

Used by the assembler to rewrite FEL expressions after AST transformations
(e.g., field path prefixing during $ref resolution).

`write_expr` and helpers serialize each [`Expr`] variant; parentheses only when needed.



## Module: types

FEL runtime value types with base-10 decimal arithmetic.

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

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &BinaryOp) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> BinaryOp`



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
- `FieldRef{ name: Option<String>, path: Vec<PathSegment> }` - `$` field reference (optional name for bare `$`).
- `VarRef{ name: String, path: Vec<PathSegment> }` - Bare identifier path (`x`, `x.a`) — no leading `$` in source.
- `ContextRef{ name: String, arg: Option<String>, tail: Vec<String> }`
- `UnaryOp{ op: UnaryOp, operand: Box<Expr>, bang: bool }`
- `BinaryOp{ op: BinaryOp, left: Box<Expr>, right: Box<Expr> }`
- `Ternary{ condition: Box<Expr>, then_branch: Box<Expr>, else_branch: Box<Expr> }` - Symbol-form conditional (`cond ? then : else`).
- `IfThenElse{ condition: Box<Expr>, then_branch: Box<Expr>, else_branch: Box<Expr> }` - Keyword-form conditional (`if cond then then_branch else else_branch`).
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
`repeatContext` / `repeat_context`, `instances`, `locale`, `meta`.

```rust
fn formspec_environment_from_json_map(ctx: &serde_json::Map<String, serde_json::Value>) -> crate::FormspecEnvironment
```

---

## Source: fel_core/convert.md

**fel_core > convert**

# Module: convert

## Contents

**Functions**

- [`fel_to_json`](#fel_to_json) - Backward-compatible alias for UI-friendly encoding (`fel_to_ui_json`).
- [`fel_to_ui_json`](#fel_to_ui_json) - Convert a `TypeValue` into UI-friendly JSON.
- [`fel_to_wire_json`](#fel_to_wire_json) - Convert a `TypeValue` to a `serde_json::Value`.
- [`field_map_from_json_str`](#field_map_from_json_str) - Parse a JSON object string into a field map (empty or `"{}"` → empty map).
- [`json_object_to_field_map`](#json_object_to_field_map) - JSON object → flat field map for FEL `MapEnvironment` (`{}` / empty → empty map).
- [`json_to_fel`](#json_to_fel) - Convert a `serde_json::Value` to a `TypeValue`.

---

## fel_core::convert::fel_to_json

*Function*

Backward-compatible alias for UI-friendly encoding (`fel_to_ui_json`).

```rust
fn fel_to_json(val: &crate::types::Value) -> serde_json::Value
```



## fel_core::convert::fel_to_ui_json

*Function*

Convert a `TypeValue` into UI-friendly JSON.

Intended for display surfaces and host APIs that do not feed values back into
FEL evaluation. Decimal values are emitted as JSON numbers only when the
serialized JSON number text round-trips exactly to the same Decimal.

```rust
fn fel_to_ui_json(val: &crate::types::Value) -> serde_json::Value
```



## fel_core::convert::fel_to_wire_json

*Function*

Convert a `TypeValue` to a `serde_json::Value`.

Conversion rules:
- `Null` → `Value::Null`
- `Boolean(b)` → `Value::Bool(b)`
- `Number(n)` → `{"$type": "number", "value": <number|string>}`. The `value`
  member is a JSON integer only when whole and within JavaScript's safe integer range;
  otherwise it is a normalized decimal string.
- `String(s)` → `Value::String(s)`
- `Date(d)` → `{"$type": "date", "value": <iso-string>}`
- `Money { amount, currency }` → `{"$type": "money", "amount": <decimal-string>, "currency": <string>}`
- `Array(arr)` → `Value::Array` (recursive)
- `Object(entries)` → `Value::Object` (recursive)

```rust
fn fel_to_wire_json(val: &crate::types::Value) -> serde_json::Value
```



## fel_core::convert::field_map_from_json_str

*Function*

Parse a JSON object string into a field map (empty or `"{}"` → empty map).

```rust
fn field_map_from_json_str(fields_json: &str) -> Result<std::collections::HashMap<String, crate::types::Value>, String>
```



## fel_core::convert::json_object_to_field_map

*Function*

JSON object → flat field map for FEL `MapEnvironment` (`{}` / empty → empty map).

```rust
fn json_object_to_field_map(val: &serde_json::Value) -> std::collections::HashMap<String, crate::types::Value>
```



## fel_core::convert::json_to_fel

*Function*

Convert a `serde_json::Value` to a `TypeValue`.

Conversion rules:
- `Null` → `TypeValue::Null`
- `Bool(b)` → `TypeValue::Boolean(b)`
- `Number(n)` → `TypeValue::Number` (tries i64, then u64, then f64)
- `String(s)` → `TypeValue::String(s)` — no silent date coercion
- `Array(arr)` → `TypeValue::Array` (recursive)
- `Object` with `"$type": "money"` + `"amount"` + `"currency"` → `TypeValue::Money`
- `Object` otherwise → `TypeValue::Object` (recursive)

Money detection requires an explicit `"$type": "money"` marker. Objects that
happen to have `amount` and `currency` fields but lack the marker are treated
as regular objects — no heuristic guessing.

The `amount` field accepts either a JSON number or a JSON string that parses
as a Decimal.

```rust
fn json_to_fel(val: &serde_json::Value) -> crate::types::Value
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

- **Clone**
  - `fn clone(self: &Self) -> Dependencies`
- **Default**
  - `fn default() -> Dependencies`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



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
- Locale via `locale()` and `pluralCategory()`
- Runtime metadata via `runtimeMeta(key)`

**Fields:**
- `data: std::collections::HashMap<String, crate::types::Value>` - Primary data dict — backs `$field` references.
- `instances: std::collections::HashMap<String, crate::types::Value>` - Named secondary instances — backs `@instance('name')`.
- `mip_states: std::collections::HashMap<String, MipState>` - MIP states per dotted field path.
- `variables: std::collections::HashMap<String, crate::types::Value>` - Definition variables — backs `@variableName`.
- `repeat_context: Option<RepeatContext>` - Current repeat context (if inside a repeat iteration).
- `current_datetime: Option<crate::types::Date>` - Current runtime date for today()/now().
- `locale: Option<String>` - Active locale code (BCP 47) — backs `locale()` and default for `pluralCategory()`.
- `meta: std::collections::HashMap<String, crate::types::Value>` - Runtime metadata bag — backs `runtimeMeta(key)`.

**Methods:**

- `fn new() -> Self` - Empty environment (no data, instances, or repeat context).
- `fn set_field(self: & mut Self, path: &str, value: TypeValue)` - Set a field value by dotted path (e.g., "address.city").
- `fn set_instance(self: & mut Self, name: &str, value: TypeValue)` - Set a named instance.
- `fn set_mip(self: & mut Self, path: &str, state: MipState)` - Set MIP state for a field path.
- `fn set_variable(self: & mut Self, name: &str, value: TypeValue)` - Set a variable value.
- `fn set_locale(self: & mut Self, code: &str)` - Set the active locale code (BCP 47).
- `fn set_meta(self: & mut Self, key: &str, value: TypeValue)` - Set a runtime metadata value.
- `fn set_now_from_iso(self: & mut Self, iso: &str)` - Set the current runtime datetime from an ISO string.
- `fn push_repeat(self: & mut Self, current: TypeValue, index: usize, count: usize, collection: Vec<TypeValue>)` - Enter a repeat context.
- `fn pop_repeat(self: & mut Self)` - Leave the current repeat context, restoring the parent.

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`
- **Environment**
  - `fn resolve_field(self: &Self, segments: &[String]) -> TypeValue`
  - `fn resolve_context(self: &Self, name: &str, arg: Option<&str>, tail: &[String]) -> TypeValue`
  - `fn mip_valid(self: &Self, path: &[String]) -> TypeValue`
  - `fn mip_relevant(self: &Self, path: &[String]) -> TypeValue`
  - `fn mip_readonly(self: &Self, path: &[String]) -> TypeValue`
  - `fn mip_required(self: &Self, path: &[String]) -> TypeValue`
  - `fn repeat_prev(self: &Self) -> TypeValue`
  - `fn repeat_next(self: &Self) -> TypeValue`
  - `fn repeat_parent(self: &Self) -> TypeValue`
  - `fn current_date(self: &Self) -> Option<TypeDate>`
  - `fn current_datetime(self: &Self) -> Option<TypeDate>`
  - `fn locale(self: &Self) -> Option<&str>`
  - `fn runtime_meta(self: &Self, key: &str) -> TypeValue`



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
- `current: crate::types::Value` - The current row value.
- `index: usize` - 1-based index within the repeat group.
- `count: usize` - Total instance count.
- `parent: Option<Box<RepeatContext>>` - Parent repeat context (for nested repeats).
- `collection: Vec<crate::types::Value>` - All rows in the collection (for prev/next navigation).

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
- [`ParseError`](#parseerror) - Lex or parse failure with optional source span (byte offsets into the expression).

**Enums**

- [`DiagnosticKind`](#diagnostickind) - Machine-readable diagnostic categories.
- [`Error`](#error) - Failure from [`crate::parse`] or fatal-style evaluation errors surfaced as `Err`.
- [`Severity`](#severity) - Diagnostic severity for tooling and JSON wire format.

**Functions**

- [`fel_diagnostics_to_json_value`](#fel_diagnostics_to_json_value) - Evaluation diagnostics as JSON objects (default `camelCase`).
- [`fel_diagnostics_to_json_value_styled`](#fel_diagnostics_to_json_value_styled) - Evaluation diagnostics as JSON objects with configurable key style.
- [`has_error_diagnostics`](#has_error_diagnostics) - Returns `true` if any diagnostic has error severity.
- [`reject_undefined_functions`](#reject_undefined_functions) - Returns `Err` when any undefined-function diagnostic is present (WASM / strict hosts).
- [`undefined_function_names_from_diagnostics`](#undefined_function_names_from_diagnostics) - Names from `undefined function: …` diagnostics (host bindings reject these as unsupported).

---

## fel_core::error::Diagnostic

*Struct*

A non-fatal diagnostic recorded during evaluation.

**Fields:**
- `severity: Severity` - Severity for hosts and JSON wire encoding.
- `message: String` - Human-readable explanation.
- `code: Option<String>` - Stable machine-readable code for lint/UI (e.g. `FEL_SUM_REJECTS_MONEY`).
- `kind: Option<DiagnosticKind>` - Machine-readable category for robust downstream handling.
- `span: Option<std::ops::Range<usize>>` - Byte range in the source expression, when known.

**Methods:**

- `fn error<impl Into<String>>(msg: impl Trait) -> Self` - Build an error-severity diagnostic.
- `fn error_coded<impl Into<String>, impl Into<String>>(code: impl Trait, msg: impl Trait) -> Self` - Build an error-severity diagnostic with a stable [`Diagnostic::code`].
- `fn warning<impl Into<String>>(msg: impl Trait) -> Self` - Build a warning-severity diagnostic.
- `fn undefined_function<impl Into<String>>(name: impl Trait) -> Self` - Build a structured undefined-function diagnostic.
- `fn type_mismatch<impl Into<String>, impl Into<String>, impl Into<String>>(fn_name: impl Trait, expected: impl Trait, got_type: impl Trait) -> Self` - Build a structured type-mismatch diagnostic (same message shape as runtime type errors).
- `fn with_span(self: Self, span: Range<usize>) -> Self` - Attaches a source span (byte offsets into the FEL source string).

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Diagnostic`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::error::DiagnosticKind

*Enum*

Machine-readable diagnostic categories.

**Variants:**
- `UndefinedFunction{ name: String }` - Function name could not be resolved in builtins or extension registry.
- `TypeMismatch{ fn_name: String, expected: String, got: String }` - Builtin or expression context expected a different runtime type.

**Traits:** Eq

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> DiagnosticKind`
- **PartialEq**
  - `fn eq(self: &Self, other: &DiagnosticKind) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::error::Error

*Enum*

Failure from [`crate::parse`] or fatal-style evaluation errors surfaced as `Err`.

**Variants:**
- `Parse(ParseError)` - Lex/parse failure (message from lexer or parser).

**Traits:** Error

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Error`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::error::ParseError

*Struct*

Lex or parse failure with optional source span (byte offsets into the expression).

[`Error`]'s [`std::fmt::Display`] output uses the `message` field.

**Fields:**
- `message: String` - Human-readable explanation.
- `span: Option<std::ops::Range<usize>>` - Byte range in the source, when known.

**Methods:**

- `fn new<impl Into<String>>(message: impl Trait) -> Self` - Parse failure with no associated span.
- `fn with_span<impl Into<String>>(span: Range<usize>, message: impl Trait) -> Self` - Parse failure with a source span.

**Traits:** Eq

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ParseError`
- **PartialEq**
  - `fn eq(self: &Self, other: &ParseError) -> bool`
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

- **PartialEq**
  - `fn eq(self: &Self, other: &Severity) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Severity`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::error::fel_diagnostics_to_json_value

*Function*

Evaluation diagnostics as JSON objects (default `camelCase`).

```rust
fn fel_diagnostics_to_json_value(diagnostics: &[Diagnostic]) -> serde_json::Value
```



## fel_core::error::fel_diagnostics_to_json_value_styled

*Function*

Evaluation diagnostics as JSON objects with configurable key style.

```rust
fn fel_diagnostics_to_json_value_styled(diagnostics: &[Diagnostic], style: crate::wire_style::JsonWireStyle) -> serde_json::Value
```



## fel_core::error::has_error_diagnostics

*Function*

Returns `true` if any diagnostic has error severity.

```rust
fn has_error_diagnostics(diagnostics: &[Diagnostic]) -> bool
```



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

**Functions**

- [`eval_with_fields`](#eval_with_fields) - Parses and evaluates FEL with a flat field map.

---

## fel_core::evaluator::eval_with_fields

*Function*

Parses and evaluates FEL with a flat field map.

```rust
fn eval_with_fields(input: &str, fields: std::collections::HashMap<String, crate::types::Value>) -> Result<EvalResult, crate::error::Error>
```

---

## Source: fel_core/evaluator/budget.md

**fel_core > evaluator > budget**

# Module: evaluator::budget

## Contents

**Structs**

- [`EvalBudget`](#evalbudget) - Hard cap on evaluation resource consumption. Exceeding any limit returns `Err(BudgetExceededKind)`.

**Enums**

- [`BudgetExceededKind`](#budgetexceededkind) - Which resource limit was hit.

---

## fel_core::evaluator::budget::BudgetExceededKind

*Enum*

Which resource limit was hit.

**Variants:**
- `Steps` - Step count exceeded [`EvalBudget::max_steps`].
- `Alloc` - Allocation exceeded [`EvalBudget::max_alloc_bytes`].
- `Deadline` - Wall-clock deadline expired.

**Traits:** Eq, Copy

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &BudgetExceededKind) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> BudgetExceededKind`



## fel_core::evaluator::budget::EvalBudget

*Struct*

Hard cap on evaluation resource consumption. Exceeding any limit returns `Err(BudgetExceededKind)`.

**Fields:**
- `max_steps: u64` - Maximum number of node evaluations before returning `BudgetExceeded { kind: Steps }`.
- `max_alloc_bytes: u64` - Approximate allocation ceiling (bytes) before returning `BudgetExceeded { kind: Alloc }`.
- `deadline: Option<std::time::Instant>` - Wall-clock deadline for interactive/UI use (clock-bound).

**Methods:**

- `fn unlimited() -> Self` - Sentinel value that never triggers a limit — used by existing `evaluate*` entry points.
- `fn min_viable() -> Self` - Smallest budget guaranteed to allow at least one evaluation step.
- `fn for_batch(steps: u64, alloc: u64) -> Self` - Batch / projection use — no deadline, limited steps and allocation.
- `fn for_interactive(deadline: Instant) -> Self` - Interactive / UI use — unlimited steps and allocation, clock-bound.
- `fn check(self: &Self, steps: u64, alloc_bytes: u64) -> Result<(), BudgetExceededKind>` - Check whether any limit has been exceeded.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> EvalBudget`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`

---

## Source: fel_core/evaluator/core.md

**fel_core > evaluator > core**

# Module: evaluator::core

## Contents

**Structs**

- [`EvalResult`](#evalresult) - Result of evaluation: a value plus any accumulated diagnostics.
- [`Evaluator`](#evaluator) - Tree-walking evaluator with `let` scopes and diagnostic collection.
- [`EvaluatorOptions`](#evaluatoroptions) - Configuration for an evaluation run.
- [`MapEnvironment`](#mapenvironment) - Flat `HashMap` environment for tests and simple hosts (no `@` context; fixed clock in default impl).

**Functions**

- [`evaluate`](#evaluate) - Evaluate an expression against an environment (no budget, no trace, no extensions).
- [`evaluate_with`](#evaluate_with) - Evaluate with full configuration via [`EvaluatorOptions`].

**Traits**

- [`Environment`](#environment) - Resolves `$` field paths, `@` context, MIP queries, repeat navigation, and clock for FEL builtins.

---

## fel_core::evaluator::core::Environment

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
- `locale`: Active locale code for `locale()` — default none (returns null).
- `runtime_meta`: Runtime metadata value for `runtimeMeta(key)` — default null.



## fel_core::evaluator::core::EvalResult

*Struct*

Result of evaluation: a value plus any accumulated diagnostics.

**Fields:**
- `value: Value` - Computed value (may be null after errors).
- `diagnostics: Vec<crate::error::Diagnostic>` - Non-fatal issues (undefined functions, type errors, etc.).

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> EvalResult`



## fel_core::evaluator::core::Evaluator

*Struct*

Tree-walking evaluator with `let` scopes and diagnostic collection.

**Generic Parameters:**
- 'a



## fel_core::evaluator::core::EvaluatorOptions

*Struct*

Configuration for an evaluation run.

**Generic Parameters:**
- 'a

**Fields:**
- `trace: Option<&'a  mut crate::trace::Trace>` - Optional trace sink — when `Some`, the evaluator records structured steps into this trace.
- `extensions: Option<&'a crate::extensions::ExtensionRegistry>` - Optional extension registry for resolving unknown function names.
- `budget: super::budget::EvalBudget` - Resource budget for this evaluation run.

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`



## fel_core::evaluator::core::MapEnvironment

*Struct*

Flat `HashMap` environment for tests and simple hosts (no `@` context; fixed clock in default impl).

**Fields:**
- `fields: std::collections::HashMap<String, Value>` - Top-level and nested values (nested via object values); keys may be dotted.
- `current_datetime: Option<Date>` - Clock source for `today()` / `now()` lookups.

**Methods:**

- `fn new() -> Self` - Empty field map.
- `fn with_fields(fields: HashMap<String, Value>) -> Self` - Pre-populated field map.
- `fn with_current_datetime(self: Self, current_datetime: Option<Date>) -> Self` - Override the environment clock used by `today()` and `now()`.

**Trait Implementations:**

- **Environment**
  - `fn resolve_field(self: &Self, segments: &[String]) -> Value`
  - `fn resolve_context(self: &Self, _name: &str, _arg: Option<&str>, _tail: &[String]) -> Value`
  - `fn current_date(self: &Self) -> Option<Date>`
  - `fn current_datetime(self: &Self) -> Option<Date>`
- **Default**
  - `fn default() -> Self`



## fel_core::evaluator::core::evaluate

*Function*

Evaluate an expression against an environment (no budget, no trace, no extensions).

```rust
fn evaluate(expr: &Expr, env: &dyn Environment) -> EvalResult
```



## fel_core::evaluator::core::evaluate_with

*Function*

Evaluate with full configuration via [`EvaluatorOptions`].

```rust
fn evaluate_with(expr: &Expr, env: &dyn Environment, options: EvaluatorOptions) -> EvalResult
```

---

## Source: fel_core/extensions/catalog.md

**fel_core > extensions > catalog**

# Module: extensions::catalog

## Contents

**Functions**

- [`builtin_function_catalog`](#builtin_function_catalog) - Slice of all built-in functions.
- [`builtin_function_catalog_for`](#builtin_function_catalog_for) - Catalog filtered to entries reachable from `package`.

---

## fel_core::extensions::catalog::builtin_function_catalog

*Function*

Slice of all built-in functions.

Names in this catalog are reserved for
[`ExtensionRegistry::register`](crate::extensions::ExtensionRegistry::register).

```rust
fn builtin_function_catalog() -> &'static [BuiltinFunctionCatalogEntry]
```



## fel_core::extensions::catalog::builtin_function_catalog_for

*Function*

Catalog filtered to entries reachable from `package`.

`Package::Formspec` returns the union of `Universal` and `Formspec` entries
(formspec hosts can call everything). `Package::Universal` returns only
`Universal` entries — appropriate for hosts that use [`crate::MapEnvironment`] or
any non-formspec [`crate::Environment`] implementation.

```rust
fn builtin_function_catalog_for(package: Package) -> impl Trait
```

---

## Source: fel_core/extensions/registry.md

**fel_core > extensions > registry**

# Module: extensions::registry

## Contents

**Structs**

- [`ExtensionRegistry`](#extensionregistry) - Registry of extension functions.

**Enums**

- [`ExtensionError`](#extensionerror) - Error type for extension registration failures.

---

## fel_core::extensions::registry::ExtensionError

*Enum*

Error type for extension registration failures.

**Variants:**
- `NameConflict(String)` - Registration rejected: name matches a reserved word or built-in function.

**Traits:** Error

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, f: & mut std::fmt::Formatter) -> std::fmt::Result`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> ExtensionError`



## fel_core::extensions::registry::ExtensionRegistry

*Struct*

Registry of extension functions.

**Methods:**

- `fn new() -> Self` - Empty registry (no custom extensions).
- `fn register<impl Into<String>, impl Fn(&[TypeValue]) -> TypeValue + Send + Sync + 'static>(self: & mut Self, name: impl Trait, min_args: usize, max_args: Option<usize>, func: impl Trait) -> Result<(), ExtensionError>` - Register an extension function.
- `fn get(self: &Self, name: &str) -> Option<&ExtensionFunc>` - Look up an extension function by name.
- `fn contains(self: &Self, name: &str) -> bool` - Check if a name is registered.
- `fn call(self: &Self, name: &str, args: &[TypeValue]) -> Option<TypeValue>` - Call an extension function with null propagation.

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`

---

## Source: fel_core/extensions/schema.md

**fel_core > extensions > schema**

# Module: extensions::schema

## Contents

**Functions**

- [`builtin_function_catalog_json_value`](#builtin_function_catalog_json_value) - Returns a JSON array of all builtin function entries (compact form, suitable for tooling that
- [`builtin_function_catalog_json_value_for`](#builtin_function_catalog_json_value_for) - `builtin_function_catalog_for(package)` rendered as a JSON array of function entries.
- [`emit_schema_json`](#emit_schema_json) - Emit the FEL function catalog as a `serde_json::Value` matching the schema at

---

## fel_core::extensions::schema::builtin_function_catalog_json_value

*Function*

Returns a JSON array of all builtin function entries (compact form, suitable for tooling that
iterates the catalog). Each entry includes a synthesized `"signature"` string for UI display.
For the full normative schema document, use [`emit_schema_json`].

```rust
fn builtin_function_catalog_json_value() -> serde_json::Value
```



## fel_core::extensions::schema::builtin_function_catalog_json_value_for

*Function*

`builtin_function_catalog_for(package)` rendered as a JSON array of function entries.
Each entry includes a synthesized `"signature"` string for UI display.

```rust
fn builtin_function_catalog_json_value_for(package: Package) -> serde_json::Value
```



## fel_core::extensions::schema::emit_schema_json

*Function*

Emit the FEL function catalog as a `serde_json::Value` matching the schema at
`formspec/schemas/fel-functions.schema.json`.

The emitted value is byte-identical (up to JSON semantic equivalence — key order in
objects doesn't matter) to the canonical schema file. The round-trip test in
`tests/schema_round_trip.rs` enforces this invariant.

```rust
fn emit_schema_json() -> serde_json::Value
```

---

## Source: fel_core/extensions/types.md

**fel_core > extensions > types**

# Module: extensions::types

## Contents

**Structs**

- [`BuiltinFunctionCatalogEntry`](#builtinfunctioncatalogentry) - Structured metadata for a built-in FEL function.
- [`Example`](#example) - One worked example attached to a built-in function.
- [`ExtensionFunc`](#extensionfunc) - A registered extension function.
- [`Parameter`](#parameter) - One parameter in a built-in function signature.

**Enums**

- [`FelType`](#feltype) - FEL type identifier used in structured catalog entries.
- [`Package`](#package) - Host-package classification for a built-in.

**Type Aliases**

- [`ExtensionFn`](#extensionfn) - Type alias for extension function implementations.

---

## fel_core::extensions::types::BuiltinFunctionCatalogEntry

*Struct*

Structured metadata for a built-in FEL function.

This is the canonical source of truth for the FEL function catalog.
Emit [`crate::extensions::emit_schema_json`] to regenerate
`formspec/schemas/fel-functions.schema.json`.

**Fields:**
- `name: &'static str` - Function name as used in FEL source.
- `category: &'static str` - Functional category. Closed enum from schema:
- `parameters: &'static [Parameter]` - Ordered parameter list. Variadic parameters must be last.
- `returns: FelType` - Return type of the function.
- `return_description: Option<&'static str>` - Clarification of the return value when `returns` alone is insufficient.
- `description: &'static str` - What the function does — behavior, edge cases, and constraints.
- `null_handling: Option<&'static str>` - How the function behaves when one or more arguments are null.
- `deterministic: bool` - False if the function can return different results for the same arguments.
- `emit_deterministic_explicitly: bool` - True if `deterministic` should be emitted explicitly in the catalog JSON even when the
- `short_circuit: bool` - True if the function evaluates arguments lazily.
- `examples: &'static [Example]` - Worked examples.
- `since_version: &'static str` - Spec version in which the function was introduced (default `"1.0"`).
- `package: Package` - Host-package classification for filtering by tooling.



## fel_core::extensions::types::Example

*Struct*

One worked example attached to a built-in function.

**Fields:**
- `expression: &'static str` - FEL expression demonstrating the function.
- `result_json: &'static str` - JSON literal for the example result, as a `&str`. Parsed to `serde_json::Value` at
- `note: Option<&'static str>` - Optional clarifying note.



## fel_core::extensions::types::ExtensionFn

*Type Alias*: `Box<dyn Fn>`

Type alias for extension function implementations.



## fel_core::extensions::types::ExtensionFunc

*Struct*

A registered extension function.

**Fields:**
- `name: String` - Human-readable name for diagnostics.
- `min_args: usize` - Minimum number of arguments.
- `max_args: Option<usize>` - Maximum number of arguments (None = unbounded).
- `func: ExtensionFn` - The implementation: receives pre-evaluated args, returns a value.



## fel_core::extensions::types::FelType

*Enum*

FEL type identifier used in structured catalog entries.

**Variants:**
- `String` - String type.
- `Number` - Number type.
- `Boolean` - Boolean type.
- `Date` - Date type (ISO 8601 date).
- `DateTime` - DateTime type (ISO 8601 dateTime).
- `Time` - Time type (HH:MM:SS).
- `Money` - Money type ({amount, currency}).
- `Array` - Array type (`array<T>`).
- `Any` - Any type (accepts or returns multiple types).
- `Null` - Null type.

**Methods:**

- `fn as_str(self: Self) -> &'static str` - Wire name used in the JSON schema.

**Traits:** Eq, Copy

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &FelType) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> FelType`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::extensions::types::Package

*Enum*

Host-package classification for a built-in.

Used by tooling (linters, IDE autocomplete) to filter the visible builtin
set per host. `Universal` builtins are reachable from any host;
`Formspec` builtins require formspec-shaped data (MIP queries, repeat
groups, instances, locale) and are no-ops against [`crate::MapEnvironment`].

**Variants:**
- `Universal` - Available to every host — pure language semantics.
- `Formspec` - Requires formspec-shaped data: MIP queries, repeat groups, instances, locale.

**Traits:** Eq, Copy

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &Package) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Package`



## fel_core::extensions::types::Parameter

*Struct*

One parameter in a built-in function signature.

**Fields:**
- `name: &'static str` - Parameter name.
- `fel_type: FelType` - FEL type of the parameter.
- `description: Option<&'static str>` - Human-readable description of the parameter.
- `required: bool` - Whether the parameter is required (default true).
- `variadic: bool` - Whether the parameter is variadic — must be last (default false).
- `allowed_values: Option<&'static [&'static str]>` - Closed set of allowed literal values (schema `enum` field).

---

## Source: fel_core/interpolation.md

**fel_core > interpolation**

# Module: interpolation

## Contents

**Functions**

- [`expr_is_interpolation_static_literal`](#expr_is_interpolation_static_literal) - True when the AST is only literals and unary `not`/`!`/`-` on such (locale §3.3.1).

---

## fel_core::interpolation::expr_is_interpolation_static_literal

*Function*

True when the AST is only literals and unary `not`/`!`/`-` on such (locale §3.3.1).

```rust
fn expr_is_interpolation_static_literal(expr: &crate::ast::Expr) -> bool
```

---

## Source: fel_core/iso_duration.md

**fel_core > iso_duration**

# Module: iso_duration

## Contents

**Enums**

- [`IsoDurationParse`](#isodurationparse) - Outcome of parsing an ISO 8601 duration for FEL.

**Functions**

- [`parse_iso8601_duration`](#parse_iso8601_duration) - Parse an ISO 8601 duration; distinguishes invalid input from out-of-range totals.
- [`parse_iso8601_duration_ms`](#parse_iso8601_duration_ms) - Parse an ISO 8601 duration to whole milliseconds.

---

## fel_core::iso_duration::IsoDurationParse

*Enum*

Outcome of parsing an ISO 8601 duration for FEL.

**Variants:**
- `Milliseconds(i64)` - Whole milliseconds (FEL `number`).
- `Invalid` - Empty input, missing `P`, unsupported shape, or a numeric component that does not fit `i128`.
- `OutOfRange` - Total milliseconds do not fit in `i64`.

**Traits:** Eq, Copy

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &IsoDurationParse) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> IsoDurationParse`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::iso_duration::parse_iso8601_duration

*Function*

Parse an ISO 8601 duration; distinguishes invalid input from out-of-range totals.

```rust
fn parse_iso8601_duration(input: &str) -> IsoDurationParse
```



## fel_core::iso_duration::parse_iso8601_duration_ms

*Function*

Parse an ISO 8601 duration to whole milliseconds.

Returns `None` on invalid input or if the result does not fit `i64`.

```rust
fn parse_iso8601_duration_ms(input: &str) -> Option<i64>
```

---

## Source: fel_core/lexer.md

**fel_core > lexer**

# Module: lexer

## Contents

**Structs**

- [`Lexer`](#lexer) - Character-based lexer over a FEL expression string.
- [`PositionedToken`](#positionedtoken) - One lexeme from [`tokenize`] for host bindings and tooling (stable type names + source span).
- [`Span`](#span) - Byte/char span in the original source (Unicode scalar indices, inclusive start, exclusive end).
- [`SpannedToken`](#spannedtoken) - A [`Token`] with its [`Span`].

**Enums**

- [`Token`](#token) - Lexical token for FEL source (literals, keywords, operators, punctuation).

**Functions**

- [`is_valid_fel_identifier`](#is_valid_fel_identifier) - Returns `true` if `s` is a valid FEL identifier: `[a-zA-Z_][a-zA-Z0-9_]*` and not a reserved keyword.
- [`sanitize_fel_identifier`](#sanitize_fel_identifier) - Sanitizes a string into a valid FEL identifier.
- [`tokenize`](#tokenize) - Tokenizes FEL source into [`PositionedToken`]s (lexical analysis only; no parse).
- [`tokenize_to_json_value`](#tokenize_to_json_value) - FEL lexer tokens as JSON for host bindings (default `camelCase`).
- [`tokenize_to_json_value_styled`](#tokenize_to_json_value_styled) - FEL lexer tokens as JSON with configurable key style.

---

## fel_core::lexer::Lexer

*Struct*

Character-based lexer over a FEL expression string.

**Generic Parameters:**
- 'a

**Methods:**

- `fn new(input: &'a str) -> Self` - Create a lexer for `input` (no allocation beyond char buffer).
- `fn tokenize(self: & mut Self) -> Result<Vec<SpannedToken>, ParseError>` - Consume the entire input and return all tokens, ending with [`Token::Eof`].



## fel_core::lexer::PositionedToken

*Struct*

One lexeme from [`tokenize`] for host bindings and tooling (stable type names + source span).

**Fields:**
- `token_type: String` - Logical token kind (e.g. `NumberLiteral`, `Identifier`).
- `text: String` - Lexeme text from the source.
- `start: usize` - Start offset in Unicode scalar indices.
- `end: usize` - End offset (exclusive) in Unicode scalar indices.

**Traits:** Eq

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> PositionedToken`
- **PartialEq**
  - `fn eq(self: &Self, other: &PositionedToken) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::lexer::Span

*Struct*

Byte/char span in the original source (Unicode scalar indices, inclusive start, exclusive end).

**Fields:**
- `start: usize` - Start offset.
- `end: usize` - End offset (exclusive).

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Span`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



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
- `Number(rust_decimal::Decimal)` - Decimal number literal.
- `StringLit(String)` - String literal (content without surrounding quotes).
- `True` - Boolean `true`.
- `False` - Boolean `false`.
- `Null` - `null` literal.
- `DateLiteral(String)` - Date literal (`@YYYY-MM-DD`).
- `DateTimeLiteral(String)` - Date-time literal (`@YYYY-MM-DDTHH:MM:SS`).
- `Identifier(String)` - Unclassified name or function identifier.
- `Let` - `let` keyword.
- `In` - `in` keyword (let binding body delimiter).
- `If` - `if` keyword.
- `Then` - `then` keyword.
- `Else` - `else` keyword.
- `And` - `and` keyword.
- `Or` - `or` keyword.
- `Not` - `not` keyword.
- `Bang` - `!` prefix (not followed by `=`). Semantically identical to `Not`
- `Plus` - `+` addition.
- `Minus` - `-` subtraction / negation.
- `Star` - `*` multiplication.
- `Slash` - `/` division.
- `Percent` - `%` modulo.
- `Ampersand` - `&` string concatenation.
- `Eq` - `=` or `==` equality.
- `NotEq` - `!=` inequality.
- `Lt` - `<` less-than.
- `Gt` - `>` greater-than.
- `LtEq` - `<=` less-than-or-equal.
- `GtEq` - `>=` greater-than-or-equal.
- `DoubleQuestion` - `??` null-coalescing operator.
- `Question` - `?` conditional (ternary) operator.
- `LParen` - `(` open parenthesis.
- `RParen` - `)` close parenthesis.
- `LBracket` - `[` open bracket.
- `RBracket` - `]` close bracket.
- `LBrace` - `{` open brace.
- `RBrace` - `}` close brace.
- `Comma` - `,` comma separator.
- `Dot` - `.` member-access dot.
- `Colon` - `:` key-value colon.
- `Dollar` - `$` field reference prefix.
- `At` - `@` context reference prefix.
- `Eof` - End-of-input sentinel.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Token`
- **PartialEq**
  - `fn eq(self: &Self, other: &Token) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::lexer::is_valid_fel_identifier

*Function*

Returns `true` if `s` is a valid FEL identifier: `[a-zA-Z_][a-zA-Z0-9_]*` and not a reserved keyword.

```rust
fn is_valid_fel_identifier(s: &str) -> bool
```



## fel_core::lexer::sanitize_fel_identifier

*Function*

Sanitizes a string into a valid FEL identifier.

- Strips characters that aren't ASCII alphanumeric or underscore.
- Prepends `_` if the result starts with a digit.
- Appends `_` if the result is a reserved keyword.
- Returns `"_"` for an empty or all-invalid input.

```rust
fn sanitize_fel_identifier(s: &str) -> String
```



## fel_core::lexer::tokenize

*Function*

Tokenizes FEL source into [`PositionedToken`]s (lexical analysis only; no parse).

```rust
fn tokenize(input: &str) -> Result<Vec<PositionedToken>, String>
```



## fel_core::lexer::tokenize_to_json_value

*Function*

FEL lexer tokens as JSON for host bindings (default `camelCase`).

```rust
fn tokenize_to_json_value(input: &str) -> Result<serde_json::Value, String>
```



## fel_core::lexer::tokenize_to_json_value_styled

*Function*

FEL lexer tokens as JSON with configurable key style.

```rust
fn tokenize_to_json_value_styled(input: &str, style: crate::wire_style::JsonWireStyle) -> Result<serde_json::Value, String>
```

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
fn parse(input: &str) -> Result<Expr, crate::error::Error>
```

---

## Source: fel_core/prepare_host.md

**fel_core > prepare_host**

# Module: prepare_host

## Contents

**Structs**

- [`PrepareHostInput`](#preparehostinput) - Inputs for [`prepare_for_host`], mirroring the engine WASM prepass.
- [`PrepareHostOptions`](#preparehostoptions) - Owned inputs for [`prepare`] after JSON / host parsing.

**Functions**

- [`host_options_from_json`](#host_options_from_json) - Parses prepare-FEL options from a JSON object (WASM / Python hosts).
- [`prepare`](#prepare) - Normalizes using owned options (convenience after [`host_options_from_json`]).
- [`prepare_for_host`](#prepare_for_host) - Applies the same normalization pass the TypeScript engine runs before WASM FEL evaluation.

---

## fel_core::prepare_host::PrepareHostInput

*Struct*

Inputs for [`prepare_for_host`], mirroring the engine WASM prepass.

**Generic Parameters:**
- 'a

**Fields:**
- `expression: &'a str` - Raw FEL expression.
- `current_item_path: &'a str` - Dotted path of the item being evaluated (may include `[n]` indices).
- `replace_self_ref: bool` - When true, bare `$` (not `$identifier`) becomes `$` + current field leaf name.
- `repeat_counts: &'a std::collections::HashMap<String, u32>` - Repeat row counts keyed by group base path (e.g. `line_items` → 2).
- `field_paths: &'a [String]` - Keys from the flat field snapshot (e.g. `rows[0].score`) used to infer repeat aliases.

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> PrepareHostInput<'a>`



## fel_core::prepare_host::PrepareHostOptions

*Struct*

Owned inputs for [`prepare`] after JSON / host parsing.

**Fields:**
- `expression: String` - Raw FEL expression.
- `current_item_path: String` - Item path for repeat / self-ref normalization.
- `replace_self_ref: bool` - When true, bare `$` becomes `$` + current field leaf name.
- `repeat_counts: std::collections::HashMap<String, u32>` - Repeat row counts by group base path.
- `field_paths: Vec<String>` - Flat field paths for repeat alias inference.

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> PrepareHostOptions`



## fel_core::prepare_host::host_options_from_json

*Function*

Parses prepare-FEL options from a JSON object (WASM / Python hosts).

```rust
fn host_options_from_json(obj: &serde_json::Map<String, serde_json::Value>) -> Result<PrepareHostOptions, String>
```



## fel_core::prepare_host::prepare

*Function*

Normalizes using owned options (convenience after [`host_options_from_json`]).

```rust
fn prepare(opts: &PrepareHostOptions) -> String
```



## fel_core::prepare_host::prepare_for_host

*Function*

Applies the same normalization pass the TypeScript engine runs before WASM FEL evaluation.

```rust
fn prepare_for_host(input: PrepareHostInput) -> String
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

## Source: fel_core/trace.md

**fel_core > trace**

# Module: trace

## Contents

**Structs**

- [`Trace`](#trace) - An ordered record of everything the evaluator emitted during one run.

**Enums**

- [`TraceStep`](#tracestep) - A single recorded event during FEL evaluation.

---

## fel_core::trace::Trace

*Struct*

An ordered record of everything the evaluator emitted during one run.

Steps are appended in evaluation order. A consumer (linter, MCP tool,
error-explainer) can render the sequence top-to-bottom to reconstruct
*why* the expression produced its result.

**Fields:**
- `steps: Vec<TraceStep>` - Steps in evaluation order.

**Methods:**

- `fn new() -> Self` - Create an empty trace.
- `fn push(self: & mut Self, step: TraceStep)` - Append a step.
- `fn len(self: &Self) -> usize` - Number of recorded steps.
- `fn is_empty(self: &Self) -> bool` - True when no steps have been recorded.

**Trait Implementations:**

- **Default**
  - `fn default() -> Trace`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Serialize**
  - `fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>`
- **Clone**
  - `fn clone(self: &Self) -> Trace`



## fel_core::trace::TraceStep

*Enum*

A single recorded event during FEL evaluation.

The variant set is intentionally narrow: only events that help explain
*why* an expression produced its result. Non-covered AST nodes omit steps
rather than emitting noise — correctness over completeness.

**Variants:**
- `FieldResolved{ path: String, value: serde_json::Value }` - A `$field` reference was resolved against the environment.
- `FunctionCalled{ name: String, args: Vec<serde_json::Value>, result: serde_json::Value }` - A function call completed and returned a value.
- `BinaryOp{ op: String, lhs: serde_json::Value, rhs: serde_json::Value, result: serde_json::Value }` - A binary operator produced a result from two operand values.
- `IfBranch{ condition_value: serde_json::Value, branch_taken: &'static str }` - A conditional (`if(...)` call or `if-then-else` / ternary) selected a branch.
- `ShortCircuit{ op: String, reason: String }` - A logical operator short-circuited; the right-hand side was not evaluated.

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> TraceStep`
- **Serialize**
  - `fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>`

---

## Source: fel_core/types.md

**fel_core > types**

# Module: types

## Contents

**Structs**

- [`CurrencyCode`](#currencycode) - ISO 4217 alphabetic currency code (three ASCII letters), normalized to uppercase.
- [`Money`](#money) - Monetary value with ISO currency code.

**Enums**

- [`Date`](#date) - Calendar date or date-time (no timezone model; used by date functions).
- [`Value`](#value) - Runtime value for FEL evaluation (mirrors JSON + dates + money).

**Functions**

- [`date_add_days`](#date_add_days) - Add days to a date.
- [`format_number`](#format_number) - Format a Decimal: strip trailing zeros, show as integer when possible.
- [`parse_date_literal`](#parse_date_literal) - Parse "@YYYY-MM-DD" into Date.
- [`parse_datetime_literal`](#parse_datetime_literal) - Parse "@YYYY-MM-DDTHH:MM:SS..." into Date.
- [`value_size_estimate`](#value_size_estimate) - Best-effort estimate of the heap footprint of a [`Value`] in bytes.

---

## fel_core::types::CurrencyCode

*Struct*

ISO 4217 alphabetic currency code (three ASCII letters), normalized to uppercase.

**Tuple Struct**: `()`

**Methods:**

- `fn parse(s: &str) -> Option<Self>` - Parses a three-letter ISO code; accepts any casing.
- `fn as_str(self: &Self) -> &str` - Uppercase ISO code slice (e.g. `USD`).

**Traits:** Eq, Copy

**Trait Implementations:**

- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &CurrencyCode) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> CurrencyCode`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::types::Date

*Enum*

Calendar date or date-time (no timezone model; used by date functions).

**Variants:**
- `Date{ year: i32, month: u32, day: u32 }` - Calendar date (year, month, day).
- `DateTime{ year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32 }` - Date with time of day (no timezone).

**Methods:**

- `fn year(self: &Self) -> i32` - Calendar year component.
- `fn month(self: &Self) -> u32` - Month 1–12.
- `fn day(self: &Self) -> u32` - Day of month.
- `fn to_naive_date(self: &Self) -> (i32, u32, u32)` - `(year, month, day)` tuple.
- `fn ordinal_days(self: &Self) -> i64` - Days since epoch (1970-01-01) for ordering.
- `fn ordinal(self: &Self) -> i64` - Full ordinal including time (seconds from epoch) for DateTime ordering.
- `fn format_iso(self: &Self) -> String` - `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS` (no timezone suffix).

**Traits:** Eq

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Date`
- **PartialEq**
  - `fn eq(self: &Self, other: &Date) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::types::Money

*Struct*

Monetary value with ISO currency code.

**Fields:**
- `amount: rust_decimal::Decimal` - Decimal amount (base-10).
- `currency: CurrencyCode` - ISO 4217 currency code (e.g. `USD`).

**Traits:** Eq

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Money`
- **PartialEq**
  - `fn eq(self: &Self, other: &Money) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::types::Value

*Enum*

Runtime value for FEL evaluation (mirrors JSON + dates + money).

**Variants:**
- `Null` - Null / absent value.
- `Boolean(bool)` - Boolean (`true` or `false`).
- `Number(rust_decimal::Decimal)` - Numeric value (high-precision decimal, rust_decimal 96-bit mantissa).
- `String(String)` - UTF-8 string value.
- `Date(Date)` - Calendar date or date-time value.
- `Array(Vec<Value>)` - Ordered list of values.
- `Object(indexmap::IndexMap<String, Value>)` - Key-value map (insertion order preserved; efficient keyed lookup).
- `Money(Money)` - Monetary amount with ISO currency code.

**Methods:**

- `fn type_name(self: &Self) -> &'static str` - Lowercase FEL type name for error messages.
- `fn is_null(self: &Self) -> bool` - True only for [`Value::Null`].
- `fn is_truthy(self: &Self) -> bool` - Loose truth test (not FEL `and`/`or` typing — used by some builtins).
- `fn as_number(self: &Self) -> Option<Decimal>` - Extract number or `None`.
- `fn as_string(self: &Self) -> Option<&str>` - Borrow string or `None`.
- `fn as_bool(self: &Self) -> Option<bool>` - Extract boolean or `None`.
- `fn as_date(self: &Self) -> Option<&Date>` - Borrow date/datetime or `None`.
- `fn as_array(self: &Self) -> Option<&Vec<Value>>` - Borrow array or `None`.
- `fn as_money(self: &Self) -> Option<&Money>` - Borrow money or `None`.

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> Value`
- **PartialEq**
  - `fn eq(self: &Self, other: &Self) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::types::date_add_days

*Function*

Add days to a date.

```rust
fn date_add_days(d: &Date, n: i64) -> Date
```



## fel_core::types::format_number

*Function*

Format a Decimal: strip trailing zeros, show as integer when possible.

```rust
fn format_number(n: rust_decimal::Decimal) -> String
```



## fel_core::types::parse_date_literal

*Function*

Parse "@YYYY-MM-DD" into Date.

Stable public API — consumed by `formspec-py` and `formspec-eval`.

```rust
fn parse_date_literal(s: &str) -> Option<Date>
```



## fel_core::types::parse_datetime_literal

*Function*

Parse "@YYYY-MM-DDTHH:MM:SS..." into Date.

Stable public API — consumed by `formspec-py` and `formspec-eval`.

```rust
fn parse_datetime_literal(s: &str) -> Option<Date>
```



## fel_core::types::value_size_estimate

*Function*

Best-effort estimate of the heap footprint of a [`Value`] in bytes.

Returns an approximation intended for allocation-budget tracking, not exact
memory accounting. The estimate accounts for the value's direct heap payload
(string length, array element counts, object entry counts) plus a small
per-element overhead estimate.

```rust
fn value_size_estimate(val: &Value) -> u64
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

- **PartialEq**
  - `fn eq(self: &Self, other: &JsonWireStyle) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> JsonWireStyle`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`

---
