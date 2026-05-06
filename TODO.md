# fel-core TODO

Audit findings from 2026-05-06 multi-agent review.

Resolved items live in [`COMPLETED.md`](COMPLETED.md).

---

## FIX NEXT (highest risk open items)

1. #6 `extensions.rs` monolith — split registry, catalog, schema
2. #7 Giant `eval_function` match — table-driven dispatch or stronger consistency checks
3. #8 Date JSON round-trip policy — confirm intent (`[superseded?]`)
4. #10 `Diagnostic` lacks source spans
5. #11 `FieldRef` vs variable refs — `VarRef` separation

### Execution protocol: true TDD refactoring loop

For each bug in `FIX NEXT`, follow this exact loop before moving to the next item:

1. **Red:** add or update a test that reproduces the current bug (must fail first).
2. **Green:** implement the smallest code change that makes the new test pass.
3. **Refactor:** clean up the touched code while keeping behavior identical.
4. **Verify:** run the focused tests for that area, then run broader FEL test suites.
5. **Iterate:** if more edge cases appear, add another failing test and repeat.

Guardrails:

- Never ship a bug fix without a failing test that proved the bug existed.
- Keep each cycle scoped to one behavioral change.
- Prefer multiple small cycles over one large patch.

---

## MEDIUM

### 6. `extensions.rs` is a 2,966-line monolith `[open]`

Combines three unrelated concerns: `ExtensionRegistry`, the 2,100-line `BUILTIN_FUNCTIONS` catalog data, and schema JSON emission.

**Fix:** Split into `extensions.rs` (registry only), `catalog.rs` (BUILTIN_FUNCTIONS data), and `schema.rs` (emit_schema_json and helpers).

### 7. Giant `eval_function` match (135 lines, ~70 arms) `[partial]`

`src/evaluator/core.rs:975-1111` — Adding a builtin requires coordinated edits in 3 files (match arm here, builtin impl in `builtins/*.rs`, catalog entry in `extensions.rs`). No compile-time check keeping them in sync.
A consistency test exists (`tests/builtin_catalog_consistency.rs`), but dispatch is still manually maintained.

**Fix:** Table-driven dispatch (`HashMap<&str, fn(...)>`) or a macro that generates the match arm, trace whitelist entry, and catalog validation from a single declaration. At minimum, add a compile-time check that all match-arm names exist in the catalog (the `builtin_catalog_consistency` test partially does this).

### 8. Date values cannot round-trip through JSON `[superseded? confirm intent]`

`src/convert.rs:131` — Dates serialize to JSON strings but deserialize back as `String`, not `Date`. Money uses `$type: "money"` tagging; Date has no equivalent.
Current convert docs describe this as intentional ("no silent date coercion"), so this may be a policy choice rather than a bug.

**Fix:** Add `$type: "date"` tagging convention in `fel_to_json` / `json_to_fel`, matching the Money pattern at `src/convert.rs:70-84`.

### 10. `Diagnostic` lacks source spans `[open]`

`src/error.rs:26-31` — Error messages are opaque strings with no source location. The parser also discards spans after AST construction (`src/parser.rs:36-53`). This limits IDE/debugger integration.

**Fix:** Add `span: Option<Range<usize>>` to `Diagnostic`. Thread spans through the evaluator. At minimum, preserve spans in the AST for future use.

### 11. `FieldRef` overloads variable references and field references `[open]`

`src/parser.rs:511-515` — Bare identifier `x` and field reference `$x` both produce `Expr::FieldRef`. A dedicated `Expr::VarRef` variant would simplify the evaluator's 4-strategy resolution at `src/evaluator/core.rs:400-481`.

**Fix:** Add `Expr::VarRef(String)` variant. Parser emits `VarRef` for bare identifiers, `FieldRef` only when `$` prefix is present.

### 13. Inconsistent diagnostic messages across builtins `[partial]`

Some builtins include function names, some don't. Some return Null silently (no diagnostic) on type mismatch (`fn_str1` at `src/evaluator/builtins/strings.rs:18-19`, `fn_str2` at line 31, `moneyAmount` at `src/evaluator/core.rs:1071-1084`), while others emit diagnostics. `src/evaluator/builtins/dates.rs:114` is the only warning-level diagnostic in the entire codebase.
Partial progress exists, but behavior is still inconsistent and should be normalized.

**Fix:** Standardize: every type mismatch should emit a diagnostic with function name and received type. Add a `fn require_type(&mut self, name, val, expected) -> bool` helper.

### 14. Code duplication across builtins `[partial]`

- Min/max comparison logic: 4 copies in `src/evaluator/builtins/aggregates.rs` (lines 65-73, 95-103, 209-217, 237-245)
- Money summation loop: 2 copies (`src/evaluator/builtins/money.rs:41-74`, `src/evaluator/builtins/aggregates.rs:255-286`)
- Date-or-string arg extraction: 3 copies in `src/evaluator/builtins/dates.rs`
- Predicate-based array iteration: 4 near-identical patterns across `aggregates.rs` and `core.rs`
- Argument count checking: 6+ scattered copies
Note: `filter_where` reduced some duplication, but broad extraction is still pending.

**Fix:** Extract shared helpers: `compare_values()`, `sum_money_values()`, `eval_date_arg()`, `iterate_where()`, `require_arg_count()`.

---

## LOW

### 15. `Object(Vec<(String,Value)>)` gives O(n) key lookup `[open]`

`src/types.rs:21` — `IndexMap` (from the `indexmap` crate) would give O(1) lookup while preserving insertion order. Also: duplicate keys are silently allowed with no validation.

### 30. `Money.currency` should be a fixed-size type `[open]`

`src/types.rs:57-62` — ISO 4217 codes are always 3 ASCII uppercase characters. A `CurrencyCode([u8; 3])` newtype with `Copy` semantics would eliminate per-instance heap allocation.

---

## TEST IMPROVEMENTS

### 38. Expand property-based testing `[partial]`

Currently only used for calendar. Missing opportunities for:

- Arithmetic commutativity/associativity
- Parse-print round-trip
- JSON round-trip
- Null propagation consistency
- Parser fuzz testing (never panics on random input)
