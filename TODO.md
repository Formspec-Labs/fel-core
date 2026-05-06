# fel-core TODO

Audit findings from 2026-05-06 multi-agent review.

---

## FIX NEXT (highest risk open items)

1. #1 Negative-number lexing breaks unspaced subtraction [completed 2026-05-06]
2. #2 ExtensionRegistry is dead code (never invoked by evaluator)
3. #5 `length(null)` null-propagation inconsistency
4. #9 Decimal precision loss in JSON serialization
5. #12 `prepare_host.rs` regex can rewrite inside string literals

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

## HIGH

### 1. Negative-number lexing breaks unspaced subtraction `[completed]`

`src/lexer.rs:238-239` — The condition `c == '-' && peek_at(1).is_ascii_digit()` lexes `-2` as a single `Number(-2)` token, making `1-2` fail to parse. The comment on line 237 says "negative via parser, not lexer" but the code does the opposite. Every subtraction without a space after `-` is broken. The parser already handles unary minus at `src/parser.rs:396-406`. All existing tests use spaces around operators, so this was never caught.

**Fix:** Remove the `c == '-' && ...` arm from the condition. The parser's `parse_unary` already transforms `-5` into `UnaryOp::Neg(Number(5))`.

**Landed (2026-05-06):**

- Added regression test `test_unspaced_subtraction` in `tests/evaluator_tests.rs` to cover `1-2` (no spaces).
- Updated `src/lexer.rs` to lex numbers only from digit-leading tokens; `-` now always lexes as `Token::Minus`.
- Verified via focused and full suite runs (`cargo test --test lexer_tests`, `cargo test --test evaluator_tests`, `cargo test`).

### 2. ExtensionRegistry is dead code — never invoked by evaluator `[completed]`

`src/evaluator/core.rs:1106-1109` — The `_ =>` fallback arm emits a diagnostic and returns Null. It never falls through to `ExtensionRegistry.call()`. Extensions can be registered but will never execute. The two-layer defense described in `src/extensions.rs:8-22` is not implemented.

The doc comment in `src/extensions.rs:8-22` contradicts the implementation: it claims the evaluator "only falls through to the extension registry for unknown names," but the dispatcher at `core.rs:1106` falls through to `Null + diagnostic` instead. Either the doc or the dispatcher is wrong.

**Fix:** Add `Option<&ExtensionRegistry>` field to `Evaluator`, attempt `registry.call(name, &evaluated_args)` in the fallback arm. (Or, if extensions are deliberately unreachable, correct the doc comment.)

**Landed (2026-05-06):**

- Added `evaluate_with_extensions(...)` API and evaluator-side optional extension registry handle.
- Unknown function fallback now evaluates args, attempts `ExtensionRegistry::call`, and only emits `undefined function` if no extension matches.
- Added regression test `test_extension_registry_fallback_executes_unknown_function`.

### 3. `sum()` silently strips currency from Money values `[partial]`

`src/evaluator/builtins/aggregates.rs:25-36` — `sum([money(10,"USD"), money(20,"USD")])` returns the number `30`, discarding currency. Mixed-currency sums would silently add raw decimals together.
The `moneySumWhere` path now enforces currency checks, but generic `sum()` still drops currency semantics.

**Fix:** Either return `Money` when all elements are same-currency money, or emit a diagnostic and return Null for money-typed arrays (direct users to `moneySum()`).

### 4. `boolean()` and `is_truthy()` have incompatible truth definitions `[open]`

`src/evaluator/builtins/logic_types.rs:138-146` vs `src/types.rs:111` — `boolean(2)` produces Null + diagnostic (only 0/1 accepted), but `is_truthy()` treats any non-zero number as true. Two codepaths, two different answers.

**Fix:** Align the definitions. Either `boolean()` accepts all non-zero numbers (matching `is_truthy()`), or document the intentional split and which builtins use which.

### 5. `length(null)` returns `0`, violating null propagation `[completed]`

`src/evaluator/builtins/strings.rs:45` — Every other builtin propagates null input as null output. `length(null) → 0` and `empty(null) → true` silently conflate absence with emptiness. Meanwhile `count(null) → Null` — inconsistent.

**Fix:** Decide: either both `length(null)` and `empty(null)` should return Null (consistent with null propagation), or both should return their "empty" defaults (convenient for forms). Document the decision. At minimum, make `length` and `count` agree.

**Landed (2026-05-06):**

- Aligned null semantics to propagation: `length(null) -> null`, `empty(null) -> null`, `present(null) -> null`.
- Updated evaluator tests and edge-case coverage to assert the new policy.

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

### 9. Decimal precision loss in JSON serialization `[completed]`

`src/convert.rs:125-128` — Non-integer Decimals go through `f64`, losing ~13 digits of precision. For a financial expression language, this matters.

**Fix:** Serialize Decimal amounts as JSON strings (e.g., `"amount": "99.99"`). `json_to_fel` already supports string amounts at `src/convert.rs:85`.

**Landed (2026-05-06):**

- Updated `fel_to_json` to serialize non-integer decimals as strings, avoiding `f64` conversion.
- Money serialization now preserves decimal precision in `"amount"` through string values when fractional.
- Added precision-focused conversion tests (including high-precision decimal roundtrip shape).

### 10. `Diagnostic` lacks source spans `[open]`

`src/error.rs:26-31` — Error messages are opaque strings with no source location. The parser also discards spans after AST construction (`src/parser.rs:36-53`). This limits IDE/debugger integration.

**Fix:** Add `span: Option<Range<usize>>` to `Diagnostic`. Thread spans through the evaluator. At minimum, preserve spans in the AST for future use.

### 11. `FieldRef` overloads variable references and field references `[open]`

`src/parser.rs:511-515` — Bare identifier `x` and field reference `$x` both produce `Expr::FieldRef`. A dedicated `Expr::VarRef` variant would simplify the evaluator's 4-strategy resolution at `src/evaluator/core.rs:400-481`.

**Fix:** Add `Expr::VarRef(String)` variant. Parser emits `VarRef` for bare identifiers, `FieldRef` only when `$` prefix is present.

### 12. `prepare_host.rs` regex can rewrite inside string literals `[completed]`

`src/prepare_host.rs:183-218` — `resolve_qualified_group_refs` does regex replacement without tracking quote state, so `$items.qty` inside a string literal would be incorrectly rewritten.

**Fix:** Add quote-state tracking (like `replace_bare_current_field_refs` does at lines 144-161) to `resolve_qualified_group_refs`, or switch to AST-based transformation.

**Landed (2026-05-06):**

- Replaced regex-only qualified group rewrite with quote-aware scanner logic that skips quoted spans.
- Added regression test `qualified_repeat_reference_inside_string_literal_is_not_rewritten`.

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

### 16. `MapEnvironment` has a hardcoded clock `[completed]`

`src/evaluator/core.rs:127-144` — Pinned to 2026-03-20. Wrong for any production use of `MapEnvironment`. Add a way to inject or use the real clock.

**Landed (2026-05-06):**

- Added configurable `current_datetime` field on `MapEnvironment`.
- Added fluent override API `with_current_datetime(...)` used by tests/hosts.
- Added regression test `test_map_environment_clock_can_be_overridden`.

### 17. All 17 modules are `pub` — many should be `pub(crate)` `[completed]`

`src/lib.rs:13-29` — Modules like `iso_duration`, `interpolation`, `wire_style`, `trace`, `context_json` are implementation details. Making them `pub(crate)` would reduce the public API surface and prevent external coupling.

**Landed (2026-05-06):**

- Tightened module visibility in `src/lib.rs` for implementation-detail modules:
  `context_json`, `interpolation`, `iso_duration`, `trace`, and `wire_style` are now `pub(crate)`.
- Public API remains available through explicit re-exports, and full test suite remains green.

### 18. `Error::Eval` variant is dead code `[completed]`

`src/error.rs:9-10` — Declared but never constructed anywhere in the codebase. Remove or document as reserved.

**Landed (2026-05-06):**

- Removed `Error::Eval` variant from `src/error.rs`.
- Updated README wording to reflect diagnostic-first evaluation failures.

### 19. Over-broad re-exports of schema types in public API `[completed]`

`src/lib.rs:47-52` — `BuiltinFunctionCatalogEntry`, `Example`, `Parameter`, `FelType`, `emit_schema_json` are schema-generation internals. Only `ExtensionRegistry`, `ExtensionError`, `ExtensionFunc`, `Package`, and catalog query functions need to be public.

**Landed (2026-05-06):**

- Removed schema-internal root re-exports from `src/lib.rs` (`BuiltinFunctionCatalogEntry`, `Example`, `Parameter`, `FelType`, `emit_schema_json`).
- Updated in-repo callsites to use module-qualified access via `fel_core::extensions::emit_schema_json`.
- Verified with full `cargo test` pass.

### 20. Tracing double-evaluates arguments for eager functions `[open]`

`src/evaluator/core.rs:336-361` — Pre-evaluates args to capture JSON values, then `eval_function` evaluates them again. Documented at `src/evaluator/util.rs:18-25`. No guard against future impure builtins being added to the whitelist.

### 21. `Ternary` and `IfThenElse` are structurally identical AST nodes `[open]`

`src/ast.rs:57-67` — Same fields, same evaluator behavior. Separate only for printer fidelity. Consider unifying with a `syntax` tag, or document the rationale.

### 22. `??` precedence is tighter than comparison (unlike JS/TS) `[completed/doc-intent]`

`src/parser.rs:13-14` — `$x ?? $y > 5` parses as `($x ?? $y) > 5`. In JS/TS, `??` has lower precedence than comparison. This is now explicitly documented in parser precedence and appears intentional.

### 23. No recursion depth limit in parser `[open]`

All parse methods are recursive with no depth tracking. Deeply nested expressions (10,000+ levels) could cause stack overflow. Add a max-depth counter.

### 24. Fractional seconds in ISO duration use f64 `[completed]`

`src/iso_duration.rs:203-214` — Accumulates digits into an `f64` then rounds. Should use integer arithmetic to avoid floating-point rounding errors.

**Landed (2026-05-06):**

- Replaced fractional-second millisecond conversion in `src/iso_duration.rs` with integer arithmetic + rounding digit.
- Added rounding-boundary regression tests (`0.9994`, `0.9995`, `1.0005` seconds cases).

### 25. `get_array` clones the entire array on every aggregate call `[open]`

`src/evaluator/core.rs:1121-1133` — Returns `Vec<Value>` by value. A borrowed slice would suffice for most builtins.

### 26. Implementation logic in `lib.rs` `[open]`

`src/lib.rs:82-195` — `token_type_name()`, `slice_by_char_offsets()`, `tokenize()`, `tokenize_to_json_value()`, `fel_diagnostics_to_json_value()`, `eval_with_fields()` all belong in domain modules (lexer, error, evaluator).

### 27. `rust_decimal::Decimal` re-export couples public API to third-party version `[completed]`

`src/lib.rs:61` — If `rust_decimal` makes a semver-incompatible change, downstream code breaks. Consider wrapping in a newtype or documenting the version pin clearly.

**Landed (2026-05-06):**

- Removed root-level `Decimal` re-export from `src/lib.rs`.
- Public API no longer directly pins downstream callers to `rust_decimal` through crate-root exports.

### 28. `PostfixAccess` inconsistent with `FieldRef` path representation `[open]`

`src/parser.rs:412-448` — Each `.field` creates a nested `PostfixAccess`, while `FieldRef` uses a flat `Vec<PathSegment>`. The evaluator must handle both, doubling path-accession code.

### 29. `wire_style.rs` used inconsistently `[open]`

`src/wire_style.rs` defines `JsonWireStyle` for dependency JSON keys, but other JSON emission functions (`tokenize_to_json_value`, `fel_diagnostics_to_json_value`) hardcode camelCase. Python hosts get inconsistent key casing.

### 30. `Money.currency` should be a fixed-size type `[open]`

`src/types.rs:57-62` — ISO 4217 codes are always 3 ASCII uppercase characters. A `CurrencyCode([u8; 3])` newtype with `Copy` semantics would eliminate per-instance heap allocation.

### 31. `Money` has `PartialEq` but not `Eq` `[completed]`

`src/types.rs:80-84` — `Date` derives `Eq` but `Money` only has manual `PartialEq`. Since `Decimal` is `Ord`, adding `Eq` is safe and consistent.

**Landed (2026-05-06):**

- `Money` now derives `PartialEq, Eq`; removed manual `PartialEq` impl.

### 32. `serde_json` default uses `BTreeMap`, losing FEL insertion order for Objects `[partial / needs verification]`

`src/convert.rs:133-138` — Object key ordering from `Vec<(String, Value)>` is lost when serialized through `serde_json::Map` (which defaults to `BTreeMap`). This claim depends on serde_json build features and should be verified with an explicit test in this workspace.

### 33. `undefined_function_names_from_diagnostics` uses string prefix matching `[open]`

`src/error.rs:74` — Matches `"undefined function: "` prefix from `core.rs:1107`. If the diagnostic message format changes, this extractor silently breaks. A structured `DiagnosticKind` enum would be more robust.

### 34. Chained comparisons produce confusing type errors `[completed]`

`src/parser.rs:259-278` — `1 < 2 < 3` parses as `(1 < 2) < 3` → `true < 3` → confusing type-error diagnostic. Either reject chained comparisons or support them natively.

**Landed (2026-05-06):**

- Parser now rejects chained comparison forms (`1 < 2 < 3`, `1 <= 2 <= 3`) with a clear parse error.
- Added parser rejection tests to lock behavior and avoid confusing runtime type diagnostics.

---

## TEST IMPROVEMENTS

### 35. Create shared test helpers module `[completed]`

`eval()`, `num()`, `dec()`, `s()`, `arr()`, `env_with()` are duplicated across 5+ test files. Create `tests/common/mod.rs`.

**Landed (2026-05-06):**

- Added shared helper module `tests/common/mod.rs` with common evaluator/value constructors.
- Migrated duplicated helper logic from `tests/evaluator_tests.rs` and `tests/evaluator_edge_cases.rs` to use the shared module.

### 36. Add unit tests for `src/error.rs` `[completed]`

Zero tests for `undefined_function_names_from_diagnostics`, `has_error_diagnostics`, `reject_undefined_functions`, `Severity::as_wire_str()`.

**Landed (2026-05-06):**

- Added direct unit tests in `src/error.rs` covering all four previously untested helpers/APIs.

### 37. Add `src/context_json.rs` tests `[open]`

Only 1 test exists. Missing coverage for meta, locale, repeat counts, MIP states, edge cases.

### 38. Expand property-based testing `[partial]`

Currently only used for calendar. Missing opportunities for:

- Arithmetic commutativity/associativity
- Parse-print round-trip
- JSON round-trip
- Null propagation consistency
- Parser fuzz testing (never panics on random input)

### 39. Remove stale BUG docs in `regex_tests.rs` `[completed]`

The hand-rolled regex engine was replaced with the `regex` crate. Stale BUG comments and limitation notes at lines 5-14, 150, and 178-179 are outdated. Tests deliberately exercising the broken-but-now-passing escape behavior should be reviewed.
Reframe this as cleanup debt (docs/comments/tests) instead of active runtime bug risk.

**Landed (2026-05-06):**

- Removed stale hand-rolled-engine BUG commentary in `tests/regex_tests.rs`.
- Kept runtime assertions intact and validated with `cargo test --test regex_tests`.

### 40. Add stress/performance tests `[open]`

No tests for very large arrays, deeply nested expressions (current deepest test is 3 levels), large field maps. No criterion benchmarks.
