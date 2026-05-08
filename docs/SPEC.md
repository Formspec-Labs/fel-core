# FEL Semantics Specification v1.0

This is the canonical, normative specification for the Formspec Expression Language (FEL). An independent implementor can produce a conformant FEL engine from this document alone.

---

## 1. Grammar

The grammar is defined in three source files that together constitute the normative parse description:

- **Lexer** — [`src/lexer.rs`](../src/lexer.rs): hand-rolled character scanner producing `SpannedToken`s. Token types defined in the `Token` enum (line 11). Entry point: `Lexer::tokenize()`.
- **Parser** — [`src/parser.rs`](../src/parser.rs): hand-rolled recursive-descent parser over the token stream. Entry point: `parse(input: &str) -> Result<Expr, Error>`.
- **AST** — [`src/ast.rs`](../src/ast.rs): the `Expr` enum (line 20) defines all expression nodes, `UnaryOp` and `BinaryOp` enums define operator discriminants, `PathSegment` (line 5) defines field-path segments.

The grammar is **not** reproduced inline here. The PEG-equivalent specification lives at `specs/fel/fel-grammar.llm.md` in the Formspec repo. The Rust parser is the reference implementation.

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

---

## 3. Null Propagation

Null propagates through most operations. When an operand is null, the operation itself yields null (and emits no additional diagnostic — the null value carries the signal).

### 3.1 Operators that propagate null

| Operator(s) | Behavior |
|-------------|----------|
| `not`, `!` | Null operand → null. |
| `-` (negation) | Null operand → null. |
| `+`, `-`, `*`, `/`, `%` | Either operand null → null (checked in `apply_binary`, `src/evaluator/core.rs:1080`). |
| `<`, `<=`, `>`, `>=` | Either operand null → null (same `apply_binary` gate). |
| `&` (concat) | Either operand null → null. |
| `and`, `or` | Left operand null → null. Additionally, if right operand evaluates to null, result is null. |
| `in`, `not in` | Left value null → null; container null → null. Container non-array → diagnostic + null. |
| Field resolution (`$x`, `$x.a`) | Unresolved field → null. Index out of bounds → null. Property access on non-object → null. |

### 3.2 Operators that do NOT propagate null

| Operator(s) | Behavior |
|-------------|----------|
| `==` (equality) | `null == null` → `true`. `null == x` (x ≠ null) → `false`. Implemented in `eval_equality` (`core.rs:1292`). |
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

The definitive function catalog is the static `BUILTIN_FUNCTIONS` slice in [`src/extensions/catalog.rs`](../src/extensions/catalog.rs). A machine-readable JSON Schema is emitted via `cargo run -p fel-core --bin emit-fel-schema` (see [`src/extensions/schema.rs`](../src/extensions/schema.rs)).

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
| `locale` | 3 | `locale`, `runtimeMeta`, `pluralCategory` |

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
