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



