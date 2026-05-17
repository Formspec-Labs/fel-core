# FEL Semantics Specification v1.0

This is the canonical, normative semantics specification for the Formspec
Expression Language (FEL). It is one member of the FEL internal-ratification
specification set:

- this document, for value, evaluation, diagnostics, host-environment, budget,
  and extension semantics;
- [`../specs/fel/fel-grammar.md`](../specs/fel/fel-grammar.md), for normative
  syntax;
- [`../conformance/fel-conformance.jsonl`](../conformance/fel-conformance.jsonl),
  for executable conformance fixtures;
- the generated builtin-function catalog schema emitted by
  `cargo run --bin emit-fel-schema`, for machine-readable builtin metadata.

A conformant implementation can be produced from the specification set without
copying the Rust reference implementation.

---

## Status of This Specification

This is a Formspec-internal W3C-style ratified specification as of 2026-05-17.
It is not a W3C Recommendation and has not been reviewed or endorsed by W3C.

The FEL 1.0 language semantics are ratified for downstream Formspec, Workspec,
and third-party evaluator implementations. The Rust crate remains
pre-1.0 as a library API: public Rust entry points may still be renamed before
crate publication, but language syntax, value semantics, builtin behavior,
diagnostic-kind wire shapes, and conformance-fixture expectations are treated as
ratified unless this document and the conformance corpus change together.

Normative requirements use the RFC 2119 keywords **MUST**, **MUST NOT**,
**SHOULD**, **SHOULD NOT**, and **MAY**.

## Normative References

- **FEL Grammar v1.0** — [`../specs/fel/fel-grammar.md`](../specs/fel/fel-grammar.md).
- **FEL Conformance Corpus** — [`../conformance/fel-conformance.jsonl`](../conformance/fel-conformance.jsonl).
- **FEL Conformance Manifest** — [`../conformance/manifest.json`](../conformance/manifest.json).
- **Builtin Function Catalog** — `src/extensions/catalog.rs`, serialized by
  `src/extensions/schema.rs` and checked by `tests/schema_round_trip.rs`.
- **Rust Reference Engine** — this crate. It is the reference implementation,
  but prose, grammar, schema, and fixtures are the normative specification set.

## Conformance Classes

A FEL implementation MAY claim one or more conformance classes:

| Class | Requirements |
|-------|--------------|
| Parser | Accepts and rejects the language defined by the normative grammar and reports approximate syntax-error position. |
| Evaluator | Implements the value model, operators, null propagation, builtin semantics, date/money/decimal behavior, and resource-budget outcomes defined here. |
| Diagnostics | Emits the diagnostic wire shape in §8 and preserves existing `DiagnosticKind` variants as append-only. |
| Host environment | Implements field, context, repeat, MIP, locale, clock, and extension-function hooks with the contracts in this document. |

An implementation claiming evaluator conformance MUST pass the public
conformance corpus for every fixture whose host-environment features it claims.
An implementation claiming parser conformance MUST also satisfy the parser
requirements in the grammar document's conformance section.

## 1. Grammar

The grammar is defined normatively in
[`../specs/fel/fel-grammar.md`](../specs/fel/fel-grammar.md). The Rust lexer,
parser, and AST are the reference implementation of that grammar:

- **Lexer** — [`src/lexer.rs`](../src/lexer.rs): hand-rolled character scanner producing `SpannedToken`s. Token types are defined in the `Token` enum. Entry point: `Lexer::tokenize()`.
- **Parser** — [`src/parser.rs`](../src/parser.rs): hand-rolled recursive-descent parser over the token stream. Entry point: `parse(input: &str) -> Result<Expr, Error>`.
- **AST** — [`src/ast.rs`](../src/ast.rs): the `Expr` enum (line 20) defines all expression nodes, `UnaryOp` and `BinaryOp` enums define operator discriminants, `PathSegment` (line 5) defines field-path segments.

If this document and the grammar document disagree on syntax, the grammar
document wins. If this document and the Rust implementation disagree on
evaluation semantics, the discrepancy is a specification or implementation bug
and MUST be resolved by updating both this document and the conformance corpus
where observable behavior changes.

Key structural points:
- Both `=` and `==` denote equality (a single `=` is not assignment).
- `!` is a parse-time synonym for `not` (except `!in` is rejected; use `not in`).
- Chained equality/comparisons (`a == b == c`, `0 < x < 10`) are **parse errors** — use explicit `and`.
- `$` field references, `@` context references, `let` bindings, `if...then...else` (keyword form), `if(...)` (function form), and ternary `? :` are all first-class AST nodes.
- String literals support single-quote and double-quote delimiters, `\n`, `\t`, `\r`, `\\`, `\"`, `\'`, and `\uXXXX` Unicode escapes.
- Line comments (`//`) and block comments (`/* */`) are supported.
- Reserved words: `true`, `false`, `null`, `let`, `in`, `if`, `then`, `else`, `and`, `or`, `not`.

---

## 2. Type System

Runtime values are defined in [`src/types.rs`](../src/types.rs). The `Value` enum (line 44) has eight variants:

| Variant | Rust payload | Notes |
|---------|-------------|-------|
| `Null` | unit | Singleton null; distinct from all other variants. |
| `Boolean` | `bool` | |
| `Number` | `rust_decimal::Decimal` | 96-bit mantissa, base-10. No NaN, no infinity, no floating-point. |
| `String` | `String` | UTF-8. |
| `Date` | `enum Date` | See below. |
| `Array` | `Vec<Value>` | Ordered, heterogeneous. |
| `Object` | `IndexMap<String, Value>` | Insertion-order-preserving key-value map. |
| `Money` | `struct Money { amount: Decimal, currency: CurrencyCode }` | Monetary value with ISO 4217 currency code. |

**`Date`** is a sub-enum with two variants:
- `Date { year: i32, month: u32, day: u32 }` — calendar date.
- `DateTime { year, month, day, hour: u32, minute: u32, second: u32 }` — date with wall-clock time (no timezone).

**`CurrencyCode`** is an opaque wrapper around a 3-byte ASCII uppercase ISO 4217 code (e.g. `USD`). Constructed via `CurrencyCode::parse(s)` which accepts any casing.

**No implicit conversions exist between types.** FEL is strict-typed.

### 2.1 JSON Encodings

FEL defines two JSON encodings:

| Encoding | Purpose | Number representation |
|----------|---------|-----------------------|
| Public/result JSON (`fel_to_json`, `fel_to_ui_json`) | Conformance fixtures, display surfaces, and host APIs that do not need type tags. | JSON number only when the emitted number text round-trips to the same FEL `Decimal`; whole integers are additionally limited to JavaScript's safe integer range. Other numbers are normalized decimal strings. |
| Typed wire JSON (`fel_to_wire_json`) | Exact machine interchange and host values that must rehydrate as FEL types. | `{"$type":"number","value":...}` where `value` is a JSON integer only for whole safe integers and is otherwise a normalized decimal string. |

Native JavaScript `BigInt` is not a FEL wire type because it is not JSON and it
does not represent fractional decimal values. Implementations MAY use `BigInt`
internally for integer-only fast paths, but conformant interop MUST preserve the
FEL base-10 decimal model and the JSON encodings above.

---

## 3. Null Propagation

Null propagates through most operations. When an operand is null, the operation itself yields null (and emits no additional diagnostic — the null value carries the signal).

### 3.1 Operators that propagate null

| Operator(s) | Behavior |
|-------------|----------|
| `not`, `!` | Null operand → null. |
| `-` (negation) | Null operand → null. |
| `+`, `-`, `*`, `/`, `%` | Either operand null → null (checked in `apply_binary`, `src/evaluator/core.rs`). |
| `<`, `<=`, `>`, `>=` | Either operand null → null (same `apply_binary` gate). |
| `&` (concat) | Either operand null → null. |
| `and`, `or` | Left operand null → null. Additionally, if right operand evaluates to null, result is null. |
| `in`, `not in` | Left value null → null; container null → null. Container non-array → diagnostic + null. |
| Field resolution (`$x`, `$x.a`) | Unresolved field → null. Index out of bounds → null. Property access on non-object → null. |

### 3.2 Operators that do NOT propagate null

| Operator(s) | Behavior |
|-------------|----------|
| `==` (equality) | `null == null` → `true`. `null == x` (x ≠ null) → `false`. Implemented in `eval_equality` (`src/evaluator/core.rs`). |
| `!=` (inequality) | Negates `==` result. `null != null` → `false`. `null != x` → `true`. |
| `??` (null coalesce) | Left operand null → evaluate and return right. Left non-null → return left. This operator exists specifically to STOP propagation. |
| `if()` / keyword `if` | `null` condition → diagnostic (`"if: condition evaluated to null"`), result is null. But the null doesn't propagate through to branches — the condition null is an error, not a passthrough. |
| `coalesce()` | Returns first non-null argument; skips null. |

### 3.3 Function calls

Functions receive evaluated arguments including nulls. Each function's null handling is documented in the function catalog. General patterns:
- Aggregate functions (`sum`, `avg`, `min`, `max`) skip null elements but return null if the array argument itself is null.
- `count()` returns the count of non-null elements; null array returns null.
- `empty()`: null → `true`.
- `present()`: null → `false` (logical inverse of `empty`).
- Cast functions: `number(null)` → null, `string(null)` → `""`, `boolean(null)` → `false`, `date(null)` → null.
- `typeOf(null)` → `"null"`.
- `isNull(null)` → `true`.

### 3.4 Array/Object literals

Array and object construction does **not** propagate null. `[1, null, 3]` produces a 3-element array whose second element is null. `{a: 1, b: null}` produces a 2-entry object. Null elements are valid members.

---

## 4. Type Coercion

### 4.1 Implicit coercion

**None.** FEL is strict-typed. Operators reject type mismatches with a diagnostic and return null. There is no implicit number↔string conversion, no truthy/falsy coercion in boolean contexts (except where explicitly documented for specific builtins).

### 4.2 Explicit cast functions

All cast functions accept null (with per-function handling) and emit a diagnostic on unparseable input:

| Function | Accepted input | Behavior |
|----------|---------------|----------|
| `number(x)` | number, string, boolean, null | Parses string; `true` → 1, `false` → 0; null → null. |
| `string(x)` | any | `null` → `""`; `Date` → ISO-8601 string; others via Display. |
| `boolean(x)` | boolean, number, string, null | `"true"` → true, `"false"` → false; zero → false, non-zero → true; null → false. |
| `date(x)` | date, string, null | Parses `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS` from string; null → null. |

### 4.3 Money arithmetic with Number

Money operators accept Number as the second operand, treating it as a same-currency amount:

| Expression | Result |
|-----------|--------|
| `money ± number` | `Money(amount ± number, currency)` |
| `money * number` | `Money(amount * number, currency)` |
| `number * money` | `Money(number * amount, currency)` |
| `money / number` | `Money(amount / number, currency)` |
| `money % number` | `Money(amount % number, currency)` |
| `money / money` | `Number(amount_a / amount_b)` (same currency required) |

Cross-currency money operations are rejected with a diagnostic. Money values cannot be compared with ordering operators (`<`, `>`, `<=`, `>=`). Only `==` and `!=` work on money.

---

## 5. Operator Precedence

Highest to lowest (from `src/parser.rs:9–21`):

| Level | Operators | Associativity |
|-------|-----------|---------------|
| 12 (highest) | postfix `.` `[...]` | left |
| 11 | unary `not`, `!`, unary `-` | right |
| 10 | `*` `/` `%` | left |
| 9 | `+` `-` `&` (concat) | left |
| 8 | `??` (null coalesce) | left |
| 7 | `in`, `not in` | left |
| 6 | `<` `>` `<=` `>=` | left |
| 5 | `=` `==` `!=` | left |
| 4 | `and` | left (short-circuit) |
| 3 | `or` | left (short-circuit) |
| 2 | ternary `? :` | right |
| 1 (lowest) | `let...in`, `if...then...else` | — |

Notes:
- `and` short-circuits: if left is `false`, right is not evaluated.
- `or` short-circuits: if left is `true`, right is not evaluated.
- `+` and `-` share a level with `&`. `&` is the string concatenation operator, not bitwise.
- Chained equality/comparison (`a == b == c`, `a < b < c`) are parse errors — use explicit `and`.
- Parentheses override precedence as usual.

---

## 6. Builtin Function Catalog

The definitive function catalog is `BUILTIN_FUNCTIONS`, assembled in
[`src/extensions/catalog.rs`](../src/extensions/catalog.rs) from the category
modules under `src/extensions/catalog/` and exposed as a slice through
`builtin_function_catalog()`. A machine-readable catalog schema is emitted via
`cargo run -p fel-core --bin emit-fel-schema` (see
[`src/extensions/schema.rs`](../src/extensions/schema.rs)).

The emitted catalog is normative builtin metadata. Implementations MUST use the
same function names, arity rules, parameter types, return types, determinism
flags, short-circuit flags, null-handling text, and package availability
markers. Implementations MAY expose the catalog through a different API shape
as long as observable evaluation behavior matches this specification and the
conformance corpus.

Function categories:

| Category | Count | Examples |
|----------|-------|----------|
| `aggregate` | 12 | `sum`, `count`, `avg`, `min`, `max`, `countWhere`, `sumWhere`, `avgWhere`, `minWhere`, `maxWhere`, `every`, `some` |
| `string` | 11 | `length`, `contains`, `startsWith`, `endsWith`, `substring`, `replace`, `upper`, `lower`, `trim`, `matches`, `format` |
| `numeric` | 5 | `round`, `floor`, `ceil`, `abs`, `power` |
| `date` | 13 | `today`, `now`, `year`, `month`, `day`, `hours`, `minutes`, `seconds`, `time`, `timeDiff`, `duration`, `dateDiff`, `dateAdd` |
| `logical` | 6 | `if`, `coalesce`, `empty`, `present`, `selected` (+ `typeof` in `type`) |
| `type` | 9 | `isNumber`, `isString`, `isDate`, `isNull`, `typeOf` + 4 cast functions (`number`, `string`, `boolean`, `date`) |
| `money` | 6 | `money`, `moneyAmount`, `moneyCurrency`, `moneyAdd`, `moneySum`, `moneySumWhere` |
| `mip` | 4 | `valid`, `relevant`, `readonly`, `required` |
| `repeat` | 3 | `prev`, `next`, `parent` (+ `instance` for multi-instance forms) |
| `locale` | 5 | `locale`, `runtimeMeta`, `pluralCategory`, `formatNumber`, `formatDate` |

Each catalog entry defines: name, category, parameter list (name, FEL type, required/optional/variadic, description, allowed enum values), return type, description, null handling semantics, determinism flag, short-circuit flag, usage examples, since-version, and package (Universal vs Formspec — controls availability in non-formspec hosts).

**Do not enumerate individual functions here.** Refer to the catalog for arity, typing, and semantics of every function.

### 6.1 Extension functions

Unknown function names are resolved via the `ExtensionRegistry` trait. Hosts register custom functions at runtime. Extension results are subject to the same allocation budget as builtins. When no registry is provided (or the registry returns `None`), an `UndefinedFunction` diagnostic is emitted.

---

## 7. EvalBudget Contract

Defined in [`src/evaluator/budget.rs`](../src/evaluator/budget.rs). Three resource dimensions, each independently enforceable:

| Dimension | Field | Type | Behavior on exceed |
|-----------|-------|------|--------------------|
| **Step count** | `max_steps: u64` | Hard cap | Each `eval()` call increments the counter. When `steps > max_steps`, the node returns `Null` and emits `"budget exceeded (steps)"`. |
| **Allocation** | `max_alloc_bytes: u64` | Best-effort cap | Tracked for string literals (`len`), array elements (`len * 16 + recursive`), object entries (`len * 40 + recursive`), let-bindings (flat 64), concat results, and extension results. Allocations use saturating addition. When `alloc_bytes > max_alloc_bytes`, the node returns `Null` and emits `"budget exceeded (alloc)"`. Estimates are NOT byte-accurate — see `value_size_estimate()` in `types.rs:204`. |
| **Deadline** | `deadline: Option<Instant>` | Wall-clock cap | Checked at each `eval()` call. When `Instant::now() >= deadline`, the node returns `Null` and emits `"budget exceeded (deadline)"`. |

Additional invariants:
- The first budget breach suppresses all subsequent breach diagnostics for that evaluation run (`budget_breached` flag).
- Existing `evaluate()` entry points use `EvalBudget::unlimited()` (all limits set to `u64::MAX`).
- `EvalBudget::min_viable()` guarantees at least 1 step and 1024 alloc bytes.
- `EvalBudget::for_batch(steps, alloc)` — no deadline, limited steps and alloc.
- `EvalBudget::for_interactive(deadline)` — clock-bound, unlimited steps and alloc.
- `eval_depth` is separately capped at `MAX_EVAL_DEPTH = 128` (stack overflow guard, not a budget dimension).

---

## 8. Diagnostic Wire Shape

Defined in [`src/error.rs`](../src/error.rs). Diagnostics are non-fatal — evaluation continues past them, but the affected node returns `Null`.

### 8.1 `Diagnostic` struct

| Field | Type | Description |
|-------|------|-------------|
| `severity` | `Severity` | `Error`, `Warning`, or `Info`. |
| `message` | `String` | Human-readable description. |
| `code` | `Option<String>` | Machine-readable stable code (e.g. `"FEL_SUM_REJECTS_MONEY"`). |
| `kind` | `Option<DiagnosticKind>` | Machine-readable structured category. |
| `span` | `Option<Range<usize>>` | Byte offsets into the source expression. |

### 8.2 `DiagnosticKind` variants

| Variant | Fields | Meaning |
|---------|--------|---------|
| `UndefinedFunction` | `name: String` | Function name not found in builtins or extension registry. |
| `TypeMismatch` | `fn_name: String, expected: String, got: String` | Builtin or expression context expected a different runtime type. |

### 8.3 Stability commitment

`DiagnosticKind` variants are **append-only through 1.0**. Existing variant shapes will not change. New variants may be added. Hosts parsing `kind` JSON should treat unknown discriminants as pass-through.

### 8.4 JSON wire format

Diagnostics serialize to JSON arrays of objects with keys: `message`, `code` (optional), `severity` (`"error"`/`"warning"`/`"info"`), `span` (optional `{start, end}`), `kind` (optional, with camelCase keys for JS hosts, snake_case for Python). See `fel_diagnostics_to_json_value()` and `fel_diagnostics_to_json_value_styled()`.

---

## 9. Rejection List

FEL **explicitly does NOT** provide:

1. **Implicit type coercion.** No `"5" + 1` or `"true"` → boolean. Casts are explicit via `number()`, `string()`, `boolean()`, `date()`.

2. **NaN or infinity.** `rust_decimal` has no representation for NaN or infinity. Division by zero returns null with a diagnostic. Overflow returns null with a diagnostic.

3. **Binary floating-point.** All numbers are base-10 Decimals (96-bit mantissa). No IEEE 754 artifacts. `0.1 + 0.2 == 0.3` holds exactly.

4. **Mutable state.** Values are immutable. Let-bindings are lexical scoping, not mutable variables. No assignment operator.

5. **Exception propagation / throw.** Errors produce null values + diagnostics, not control-flow exceptions. Evaluation always completes; the caller inspects `EvalResult.diagnostics`.

6. **Side effects.** FEL evaluation is pure. No I/O, no network, no filesystem access. Date/time functions (`today()`, `now()`) read from the `Environment` clock, which is injected by the host. Extension functions are the integration point for host-mediated effects but are architecturally bounded by the `EvalBudget`.

7. **User-defined functions.** Only the `ExtensionRegistry` mechanism exists for host-provided functions. There is no `def` or `fn` construct in FEL grammar.

---

## Extensibility and Versioning

FEL 1.0 reserves syntax and wire shapes conservatively:

- `|>` is reserved by the grammar and MUST be rejected in v1.0.
- `DiagnosticKind` is append-only. Existing kind names and field shapes MUST NOT
  change inside the 1.0 line.
- Builtin function names are reserved. Extension functions MAY supplement but
  MUST NOT override builtins.
- New builtin functions MAY be added when the catalog schema, conformance
  corpus, and version notes are updated together.
- Host APIs MAY add convenience wrappers, but language-level observable
  behavior is governed by this specification set.

## Security Considerations

FEL evaluation is pure: it has no grammar-level I/O, network, filesystem,
process, mutation, reflection, or user-defined-function capability. Hosts MUST
keep extension functions inside the same resource-budget discipline as builtins.

Implementations SHOULD enforce parser depth and evaluator budget limits
equivalent to the reference implementation to avoid stack exhaustion,
unbounded allocation, and long-running expressions. Host applications MUST treat
FEL source as untrusted input and MUST NOT let diagnostics reveal secrets from
the host environment.

## Privacy Considerations

FEL expressions can inspect only values supplied by the host environment. Hosts
SHOULD minimize the environment passed to evaluation and avoid exposing fields,
runtime metadata, locale data, or instance data that are not required by the
expression being evaluated.

Diagnostics and traces can include source snippets, field paths, function names,
and serialized values. Hosts that persist or transmit diagnostics/traces SHOULD
apply the same privacy controls used for form data.

## Internationalization Considerations

FEL string values are Unicode strings. Identifiers are ASCII-only in v1.0.
Locale-sensitive behavior is limited to explicit locale functions:
`locale()`, `pluralCategory()`, `formatNumber()`, and `formatDate()`.
`pluralCategory()` uses CLDR-style cardinal categories through the reference
plural-rules implementation. `formatNumber()` and `formatDate()` define a
small, deterministic cross-runtime formatting subset for conformance fixtures;
they are not a full CLDR/ICU replacement.

For `formatNumber()`, `en` and all unsupported/unknown tags use comma grouping
and dot decimals; `fr`, `de`, `es`, and `it` use space grouping and comma
decimals. For `formatDate()`, `short` uses `M/D/YY` for `en`/fallback and
`DD/MM/YY` for `fr`; `medium`, `long`, and `full` use English month names for
`en`/fallback and French month names for `fr`. This intentionally small subset
is the portable FEL 1.0 behavior captured by the public conformance corpus.
Hosts that need broader locale, calendar, numbering-system, or timezone
semantics MUST supply them outside FEL or through explicit extension functions.
Date/time values do not carry a timezone.
