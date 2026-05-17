# fel-core — completed work

Archive of resolved audit items (source audit: 2026-05-06 multi-agent review).  
For open work see [`TODO.md`](TODO.md). A compact **Delivered ID** table (cross-stack summary) is at the [end of this file](#delivered-id-quick-reference-audit-initiative).

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

### 3. `sum()` silently strips currency from Money values `[completed]`

`src/evaluator/builtins/aggregates.rs:25-36` — `sum([money(10,"USD"), money(20,"USD")])` returns the number `30`, discarding currency. Mixed-currency sums would silently add raw decimals together.
The `moneySumWhere` path now enforces currency checks, but generic `sum()` still drops currency semantics.

**Fix:** Either return `Money` when all elements are same-currency money, or emit a diagnostic and return Null for money-typed arrays (direct users to `moneySum()`).

**Landed (2026-05-06):**

- `sum(money[])` no longer strips currency into raw numbers.
- Behavior is now explicit: emit diagnostic and return `Null` with guidance to use `moneySum()`.
- Added regression test `test_sum_rejects_money_array_with_diagnostic`.

### 4. `boolean()` and `is_truthy()` have incompatible truth definitions `[completed]`

`src/evaluator/builtins/logic_types.rs:138-146` vs `src/types.rs:111` — `boolean(2)` produces Null + diagnostic (only 0/1 accepted), but `is_truthy()` treats any non-zero number as true. Two codepaths, two different answers.

**Fix:** Align the definitions. Either `boolean()` accepts all non-zero numbers (matching `is_truthy()`), or document the intentional split and which builtins use which.

**Landed (2026-05-06):**

- Updated `boolean(number)` conversion to match `is_truthy()` semantics: `0 -> false`, non-zero -> `true`.
- Added regression assertions for positive and negative non-zero numeric inputs.

### 5. `length(null)` returns `0`, violating null propagation `[completed]`

`src/evaluator/builtins/strings.rs:45` — Every other builtin propagates null input as null output. `length(null) → 0` and `empty(null) → true` silently conflate absence with emptiness. Meanwhile `count(null) → Null` — inconsistent.

**Fix:** Decide: either both `length(null)` and `empty(null)` should return Null (consistent with null propagation), or both should return their "empty" defaults (convenient for forms). Document the decision. At minimum, make `length` and `count` agree.

**Landed (2026-05-06):**

- Aligned null semantics to propagation: `length(null) -> null`, `empty(null) -> null`, `present(null) -> null`.
- Updated evaluator tests and edge-case coverage to assert the new policy.

---

## MEDIUM

### 9. Decimal precision loss in JSON serialization `[completed]`

`src/convert.rs:125-128` — Non-integer Decimals go through `f64`, losing ~13 digits of precision. For a financial expression language, this matters.

**Fix:** Serialize Decimal amounts as JSON strings (e.g., `"amount": "99.99"`). `json_to_fel` already supports string amounts at `src/convert.rs:85`.

**Landed (2026-05-06):**

- Updated `fel_to_json` to serialize non-integer decimals as strings, avoiding `f64` conversion.
- Money serialization now preserves decimal precision in `"amount"` through string values when fractional.
- Added precision-focused conversion tests (including high-precision decimal roundtrip shape).

### 12. `prepare_host.rs` regex can rewrite inside string literals `[completed]`

`src/prepare_host.rs:183-218` — `resolve_qualified_group_refs` does regex replacement without tracking quote state, so `$items.qty` inside a string literal would be incorrectly rewritten.

**Fix:** Add quote-state tracking (like `replace_bare_current_field_refs` does at lines 144-161) to `resolve_qualified_group_refs`, or switch to AST-based transformation.

**Landed (2026-05-06):**

- Replaced regex-only qualified group rewrite with quote-aware scanner logic that skips quoted spans.
- Added regression test `qualified_repeat_reference_inside_string_literal_is_not_rewritten`.

---

## LOW

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

### 20. Tracing double-evaluates arguments for eager functions `[completed]`

`src/evaluator/core.rs:336-361` — Pre-evaluates args to capture JSON values, then `eval_function` evaluates them again. Documented at `src/evaluator/util.rs:18-25`. No guard against future impure builtins being added to the whitelist.

**Landed (2026-05-06):**

- Removed tracing pre-evaluation pass from `Expr::FunctionCall` and replaced it with per-call argument caching in `Evaluator`.
- `eval_arg` now memoizes values for the active traced eager function call, so each arg expression is evaluated at most once.
- `FunctionCalled` trace steps now source arg JSON from cached runtime values after the real evaluation path.
- Added regression test `traced_eager_call_evaluates_each_argument_once` using a counting `Environment` to assert no double evaluation.
- Updated eager-traceable whitelist docs in `src/evaluator/util.rs` to reflect the new single-pass behavior.
- Verified with focused trace test and full `cargo test`.

### 21. `Ternary` and `IfThenElse` are structurally identical AST nodes `[completed/doc-intent]`

`src/ast.rs:57-67` — Same fields, same evaluator behavior. Separate only for printer fidelity. Consider unifying with a `syntax` tag, or document the rationale.

**Landed (2026-05-06):**

- Kept both nodes and documented rationale directly in `src/ast.rs`:
  - `Expr::Ternary` preserves symbol-form source (`? :`).
  - `Expr::IfThenElse` preserves keyword-form source (`if/then/else`).
- Existing parser coverage already asserts distinct AST variants for both forms.

### 22. `??` precedence is tighter than comparison (unlike JS/TS) `[completed/doc-intent]`

`src/parser.rs:13-14` — `$x ?? $y > 5` parses as `($x ?? $y) > 5`. In JS/TS, `??` has lower precedence than comparison. This is now explicitly documented in parser precedence and appears intentional.

### 23. No recursion depth limit in parser `[completed]`

All parse methods are recursive with no depth tracking. Deeply nested expressions (10,000+ levels) could cause stack overflow. Add a max-depth counter.

**Landed (2026-05-06):**

- Added parser recursion-depth tracking in `src/parser.rs` with a hard maximum depth of 32 nested expression frames.
- Parser now returns a parse error (`expression nesting exceeds maximum depth ...`) instead of recursing until stack overflow.
- Added regression test `overly_deep_nesting_rejected` in `tests/parser_rejection_tests.rs`.
- Verified with focused parser test + full `cargo test`.

### 24. Fractional seconds in ISO duration use f64 `[completed]`

`src/iso_duration.rs:203-214` — Accumulates digits into an `f64` then rounds. Should use integer arithmetic to avoid floating-point rounding errors.

**Landed (2026-05-06):**

- Replaced fractional-second millisecond conversion in `src/iso_duration.rs` with integer arithmetic + rounding digit.
- Added rounding-boundary regression tests (`0.9994`, `0.9995`, `1.0005` seconds cases).

### 25. `get_array` clones the entire array on every aggregate call `[completed]`

`src/evaluator/core.rs:1121-1133` — Returns `Vec<Value>` by value. A borrowed slice would suffice for most builtins.

**Landed (2026-05-06):**

- Refactored `get_array` to return borrowed slices (`&[Value]`) instead of cloning arrays.
- Updated aggregate/money/filter callsites to iterate borrowed elements while cloning only where required for scope rebinding.
- Verified with full `cargo test` run.

### 26. Implementation logic in `lib.rs` `[completed]`

`src/lib.rs:82-195` — `token_type_name()`, `slice_by_char_offsets()`, `tokenize()`, `tokenize_to_json_value()`, `fel_diagnostics_to_json_value()`, `eval_with_fields()` all belong in domain modules (lexer, error, evaluator).

**Landed (2026-05-06):**

- Moved `PositionedToken`, host `tokenize` / `tokenize_to_json_value*` into `src/lexer.rs` (wire-style JSON via `crate::wire_style`).
- Moved `fel_diagnostics_to_json_value*` into `src/error.rs`.
- Moved `eval_with_fields` into `src/evaluator/mod.rs`.
- `lib.rs` re-exports only; wire-style tests live next to lexer and error implementations.

### 27. `rust_decimal::Decimal` re-export couples public API to third-party version `[completed]`

`src/lib.rs:61` — If `rust_decimal` makes a semver-incompatible change, downstream code breaks. Consider wrapping in a newtype or documenting the version pin clearly.

**Landed (2026-05-06):**

- Removed root-level `Decimal` re-export from `src/lib.rs`.
- Public API no longer directly pins downstream callers to `rust_decimal` through crate-root exports.

### 28. `PostfixAccess` inconsistent with `FieldRef` path representation `[completed]`

`src/parser.rs:412-448` — Each `.field` creates a nested `PostfixAccess`, while `FieldRef` uses a flat `Vec<PathSegment>`. The evaluator must handle both, doubling path-accession code.

**Landed (2026-05-06):**

- Refactored parser postfix construction to normalize path representation:
  - Appends postfix segments directly into `Expr::FieldRef.path` when the base is a field/identifier.
  - Flattens chained postfix operations into a single `Expr::PostfixAccess` node for non-field bases.
- Added parser regressions:
  - `bare_identifier_postfix_is_flat_field_ref`
  - `parenthesized_expression_postfix_stays_single_postfix_node`
- Verified with focused postfix tests and full `cargo test`.

### 29. `wire_style.rs` used inconsistently `[completed]`

`src/wire_style.rs` defines `JsonWireStyle` for dependency JSON keys, but other JSON emission functions (`tokenize_to_json_value`, `fel_diagnostics_to_json_value`) hardcode camelCase. Python hosts get inconsistent key casing.

**Landed (2026-05-06):**

- Added styled JSON emitters in `src/lib.rs`:
  - `tokenize_to_json_value_styled(input, style)`
  - `fel_diagnostics_to_json_value_styled(diagnostics, style)`
- Kept existing functions (`tokenize_to_json_value`, `fel_diagnostics_to_json_value`) as backward-compatible camelCase defaults.
- `tokenize_to_json_value_styled` now emits `tokenType` for `JsCamel` and `token_type` for `PythonSnake`.
- Added unit tests covering style-specific token key casing and diagnostic output parity.
- Verified with targeted style tests and full `cargo test`.

*(Note: styled emitters now live in `lexer` / `error` modules after item #26; crate root re-exports unchanged.)*

### 31. `Money` has `PartialEq` but not `Eq` `[completed]`

`src/types.rs:80-84` — `Date` derives `Eq` but `Money` only has manual `PartialEq`. Since `Decimal` is `Ord`, adding `Eq` is safe and consistent.

**Landed (2026-05-06):**

- `Money` now derives `PartialEq, Eq`; removed manual `PartialEq` impl.

### 32. `serde_json` default uses `BTreeMap`, losing FEL insertion order for Objects `[completed]`

`src/convert.rs:133-138` — Object key ordering from `Vec<(String, Value)>` is lost when serialized through `serde_json::Map` (which defaults to `BTreeMap`). This claim depends on serde_json build features and should be verified with an explicit test in this workspace.

**Landed (2026-05-06):**

- Enabled `serde_json` `preserve_order` feature in `Cargo.toml` so `serde_json::Map` preserves insertion order.
- Added conversion regression test `object_serialization_preserves_entry_order` in `src/convert.rs`.
- Verified with focused ordering test and full `cargo test`.

### 33. `undefined_function_names_from_diagnostics` uses string prefix matching `[completed]`

`src/error.rs:74` — Matches `"undefined function: "` prefix from `core.rs:1107`. If the diagnostic message format changes, this extractor silently breaks. A structured `DiagnosticKind` enum would be more robust.

**Landed (2026-05-06):**

- Added structured diagnostic typing via `DiagnosticKind`, including `UndefinedFunction { name }`.
- Added `Diagnostic::undefined_function(name)` constructor and switched evaluator unknown-function emission to use it.
- Updated extractor logic to prefer structured kind data, with legacy message-prefix fallback for backward compatibility.
- Added unit test covering legacy-prefix fallback behavior.
- Verified with focused `error` tests and full `cargo test`.

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

### 37. Add `src/context_json.rs` tests `[completed]`

Only 1 test exists. Missing coverage for meta, locale, repeat counts, MIP states, edge cases.

**Landed (2026-05-06):**

- Expanded `src/context_json.rs` unit coverage with focused tests for:
  - snake_case key aliases (`now_iso`, `mip_states`, `repeat_context`)
  - variables, instances, locale, and meta ingestion
  - repeat parent-chain construction + default index/count/collection handling
  - default MIP values when partial state objects are provided
- Verified with focused `context_json` tests and full `cargo test`.

### 39. Remove stale BUG docs in `regex_tests.rs` `[completed]`

The hand-rolled regex engine was replaced with the `regex` crate. Stale BUG comments and limitation notes at lines 5-14, 150, and 178-179 are outdated. Tests deliberately exercising the broken-but-now-passing escape behavior should be reviewed.
Reframe this as cleanup debt (docs/comments/tests) instead of active runtime bug risk.

**Landed (2026-05-06):**

- Removed stale hand-rolled-engine BUG commentary in `tests/regex_tests.rs`.
- Kept runtime assertions intact and validated with `cargo test --test regex_tests`.

### 40. Add stress/performance tests `[completed]`

No tests for very large arrays, deeply nested expressions (current deepest test is 3 levels), large field maps. No criterion benchmarks.

**Landed (2026-05-06):**

- Added integration suite `tests/stress_tests.rs`: large array literal `sum`, 500-key field map lookup, parentheses nesting under parser limit, bounded flat `+` chain (left-deep AST stays within eval stack), long-input tokenization.
- Left-deep addition chains must stay modest until evaluation becomes iterative; see test comment.
- Verified with `cargo test --test stress_tests` and full `cargo test`.

---

## Post-audit maintenance

### Extensions module split (TODO #6) `[completed]`

Single `src/extensions.rs` (~3k lines) mixed catalog data, schema emission, types, and `ExtensionRegistry`.

**Landed (2026-05-06):**

- Replaced with `src/extensions/mod.rs` plus `types.rs`, `catalog.rs` (`RESERVED_WORDS`, `BUILTIN_FUNCTIONS`, catalog iterators), `schema.rs` (`emit_schema_json`, UI catalog JSON helpers), `registry.rs` (`ExtensionRegistry`, `ExtensionError`).
- `fel_core::extensions::*` public paths unchanged.
- Docs: `README.md` module map and schema regeneration instructions point at `catalog.rs` for `BUILTIN_FUNCTIONS`.
- Verified with full `cargo test` and `tests/schema_round_trip.rs`.

---

## Backlog sweep (TODO #7–#15, #30, #38 — stack integration 2026-05-06)

Resolved items from [`TODO.md`](TODO.md) backlog sequencing. Cross-repo touch-ups (formspec-core, wos-lint, formspec-py, `work-spec` **`wos-core`**) landed alongside fel-core.

### 7. Builtin dispatch vs catalog drift `[completed]`

**Problem:** `tests/builtin_catalog_consistency.rs` could skip catalog names when `parse(name)` failed, hiding drift between `eval_function` and `extensions/catalog.rs`.

**Landed:**

- Tightened consistency test: unexpected parse failures fail the test instead of silent `continue`.
- Explicit allowlist reserved for names intentionally not callable as `ident()`.
- Documented that `is_eager_traceable_function` (`evaluator/util.rs`) is not the full builtin list.

---

### 10 (phase 1). `Diagnostic` spans on wire `[completed]`

**Problem:** Diagnostics had no byte range for editors or structured hosts.

**Landed:**

- `Diagnostic::span: Option<Range<usize>>` plus builder helpers in `src/error.rs`.
- `fel_diagnostics_to_json_value*` emits `span: { start, end }` when present; unit tests lock JSON shape.

**Still tracked as open in [`TODO.md`](TODO.md):** lexer-backed parse-error spans; optional `Expr` / evaluator frame spans; optional `kind` on JSON wire.

---

### 11. `Expr::VarRef` vs `$` `FieldRef` `[completed]`

**Problem:** Bare identifiers and `$` refs shared `FieldRef`; printer always emitted `$`, losing source fidelity.

**Landed:**

- `Expr::VarRef { name, path }` in `src/ast.rs`; parser assigns bare identifiers to `VarRef`, `parse_field_ref` keeps `FieldRef`.
- Evaluator, `dependencies.rs`, `printer.rs` updated; postfix/access paths aligned with `FieldRef`.
- Downstream: `formspec-core` (`fel_analysis`, `fel_condition_group_lift`), `wos-lint` (`fel_analysis`), exhaustive matches updated.

---

### 13. Builtin diagnostics normalization `[completed]`

**Problem:** Mixed silent `Null`, ad hoc strings, and inconsistent arity messaging.

**Landed:**

- Extended `Evaluator::reject_expected_type` and arity helpers (`require_exact_args`, etc.).
- `fn_round`, `fn_power`, casts in `logic_types` (`number` / `boolean` / `date`, `if` condition), `selected` (non-array → diagnostic + null); `if` exact arity where required.
- Regression coverage in `tests/evaluator_tests.rs` (e.g. type/arity diagnostics).

---

### 14. Builtin helper deduplication `[completed]`

**Problem:** Repeated manual arity checks and scattered patterns across builtins.

**Landed:**

- Consolidation in `evaluator/builtins/helpers.rs` and `Evaluator` (`require_min_args`, `require_exact_args`, …).
- Migrated sites including `if` and related clusters; further cleanup is incremental.

---

### 15. `Value::Object` as `IndexMap` `[completed]`

**Problem:** `Vec<(String, Value)>` implied linear lookup for nested access.

**Landed:**

- `Value::Object(IndexMap<String, Value>)` in `src/types.rs`; `MapEnvironment` / `access_path` and JSON convert paths use map semantics.
- **Breaking** for downstream exhaustive `Value` matches; fixed in-repo (e.g. **`wos-core/src/context.rs`** — `EvalContext::to_fel_environment` builds `IndexMap` for case/event/instance objects).
- `tests/common/mod.rs` helper `obj()` for integration tests.

---

### 30. `Money.currency` as `CurrencyCode` `[completed]`

**Problem:** Heap `String` per money value; no fixed ISO 4217 representation.

**Landed:**

- `CurrencyCode([u8; 3])` with `parse`, `as_str`, `Display`; `Money { currency: CurrencyCode }`.
- Builtins (`money.rs`), `convert.rs`, formspec-py `convert.rs` (valid ISO codes; invalid → null where appropriate).

---

### 38 (phase 1). FEL property tests beyond calendar `[completed]`

**Problem:** Only `tests/civil_calendar_proptest.rs` used `proptest` for fel-core.

**Landed:**

- `tests/fel_proptest.rs`: integer parse↔print round-trip, null propagation / equality checks, JSON scalar + shallow object round-trip via `json_to_fel` / `fel_to_json`.

**Still optional ([`TODO.md`](TODO.md)):** random-input parser fuzz for panic hunting.

---

## Open backlog R1–R20 (2026-05-06 follow-up swarm) `[completed]`

**Source:** rows formerly in [`TODO.md`](TODO.md) (plan signed-off same day).

**Landed (2026-05-06):**

- **R1 / tracing docs:** Pure-function requirement documented on [`is_eager_traceable_function`](src/evaluator/util.rs); mirrored at traced [`Expr::FunctionCall`](src/evaluator/core.rs).
- **R2:** [`sum()`](src/evaluator/builtins/aggregates.rs) rejects any array containing `Money` (including mixed with numbers); regression test.
- **R3:** [`CallArgCache`](src/evaluator/core.rs) slice-pointer contract documented.
- **R4:** Fractional-second digit policy in [`iso_duration`](src/iso_duration.rs) + README.
- **R5:** Parser rejects chained `==`/`!=` and aligns chained relational rejection ([`parse_equality`](src/parser.rs), [`parse_comparison`](src/parser.rs)); tests in [`tests/parser_rejection_tests.rs`](tests/parser_rejection_tests.rs).
- **R6:** README documents **32** nesting depth limit.
- **R7:** Arity helper message **`requires at least {n} arguments`** ([`require_min_args`](src/evaluator/core.rs)); tests/comments updated.
- **R8:** [`evaluate_with_trace_and_extensions`](src/evaluator/core.rs); extension fallback emits [`TraceStep::FunctionCalled`](src/trace.rs) when tracing; re-exported from [`lib.rs`](src/lib.rs).
- **R9 / R10 / façade:** README sections for diagnostics, limits, ISO fractions, crate-root API; **`pub use indexmap::IndexMap`** at crate root.
- **R11:** Explicit [`Expr::VarRef`](src/interpolation.rs) arm in `expr_is_interpolation_static_literal`.
- **R12:** VarRef vs `$` parity test ([`tests/evaluator_tests.rs`](tests/evaluator_tests.rs)).
- **R13:** [`CurrencyCode::as_str`](src/types.rs) `# Panics` docs.
- **R14:** AST ternary / keyword-if non-boolean condition uses [`reject_expected_type("if", ...)`](src/evaluator/core.rs).
- **R15:** [`DiagnosticKind::TypeMismatch`](src/error.rs) + JSON wire; [`reject_expected_type`](src/evaluator/core.rs) / [`diag_expected_type`](src/evaluator/core.rs); [`get_array`](src/evaluator/core.rs) uses structured mismatch.
- **R16:** [`test_undefined_function`](tests/evaluator_tests.rs) asserts kind + JSON shape (`serde_json` dev-dep).
- **R17:** Shared [`diag_expected_type`](src/evaluator/core.rs) (implemented together with R15).
- **R18:** [`eval_date_operand`](src/evaluator/builtins/dates.rs) diagnostics for wrong types and invalid date strings; [`fn_date_part`](src/evaluator/builtins/dates.rs) threading.
- **R19:** [`tests/stress_tests.rs`](tests/stress_tests.rs) parse+eval cross-check on long flat addition chain.
- **R20:** [`tests/environment_integration_tests.rs`](tests/environment_integration_tests.rs) — `formspec_environment_from_json_map` + evaluate.

**Verify:** `cargo test -p fel-core`; `cargo test -p formspec-core` and `cargo test -p wos-core` from sibling workspaces passed against these changes.

---

## Delivered ID quick reference (audit initiative)

Moved from [`TODO.md`](TODO.md) — short index of closed backlog IDs and related stack work (no duplicate narrative; see sections above for detail).

| ID | Outcome |
|----|---------|
| **#10** | `ParseError { message, span }` on `Error::Parse`; lexer/parser spans; diagnostic JSON includes **`kind`** when set ([`src/error.rs`](src/error.rs), [`JsonWireStyle`](src/wire_style.rs)). Optional evaluator/AST spans **not** implemented (defer until a concrete consumer). |
| **#8** | Date ↔ JSON policy in [`README.md`](README.md) and [`convert`](src/convert.rs) tests (`string_no_date_coercion`). |
| **#38** | Parser panic smoke test: [`tests/parser_parse_proptest.rs`](tests/parser_parse_proptest.rs) (also noted in [`README.md`](README.md) Tests). |

**formspec-core (same initiative):** `FelAnalysisError.span`; `fel_analysis_to_json_value` → `errors[]` as `{ message, span }`; condition-group lift adds `span` on parse failure; `fel_rewrite_exact` uses span-aligned parse errors.

**Cross-stack bindings (2026-05 — no fel-core code changes):** TypeScript engine normalizes WASM `errors[]` to `line`/`column`/`offset` + optional `span`; Python `analyze_expression` doc + native test; rustdoc/API.llm drift fixes; optional wos-lint parse messages append char span; formspec-wasm `analyzeFEL` snapshot test. Optional evaluator/AST spans on every subexpression remain **out of scope** here (same as **#10**).

---

## Chaos / pressure / edge-case initiative (C1–C10)

Seed: 2026-05-07. Frame = harden the foundational expression-language substrate against semantic regression, cross-runtime divergence, and resource exhaustion before downstream consumers (formspec engine TS, formspec-py, wos-server, case-portal) calcify around the current behavior.

Architectural prerequisite chain (sequence is design-driven, not calendar-driven):

```
C4 (EvalBudget seam) → C1 (AST proptest strategies) → C2, C3, C5 (properties built on C1)
C6, C7 (fuzz pipeline)         independent
C8 (coverage audit)            run last; validates prior coverage
C9, C10 (cheap insurance)      independent
```

### C1. AST-generative proptest strategies `[done]`

`src/testing/strategies.rs` behind `cfg(any(test, feature = "proptest-strategies"))`. `arb_value`, `arb_expr`, `arb_decimal` composed for structural shrinking. Tests in `tests/ast_proptest.rs`.

Properties delivered (`tests/ast_proptest.rs`):
1. `parse(print(ast)) == ast` — printer/parser fixpoint over full AST.
2. `eval(parse(s), env) == eval(parse(s), env)` — determinism across two evaluations.
3. No panic on `tokenize/parse/print/eval` for any generated AST.
4. Every `Decimal` operation in eval returns `Ok(_)` or a typed error.

### C2. Semantic invariant property table `[done]`

### C3. Cross-runtime differential oracle `[done]`

### C4. Resource-budget enforcement (`EvalBudget` seam) `[done]`

### C5. Decimal property coverage extension `[done]`

### C6. Fuzz-to-regression pipeline `[done]`

### C7. Conformance corpus as fuzz seed + dictionary `[done]`

### C8. Coverage-guided gap audit `[done]`

### C9. Concurrency smoke `[done]`

### C10. Snapshot error messages `[done]`

`tests/parser_rejection_tests.rs` and evaluator diagnostic paths produce error prose that nothing previously pinned. Added `insta` as dev-dep; snapshot every error message produced by the rejection suite and the evaluator diagnostic suite.

---

## 2026-05-08 Multi-lens review follow-ups (R21–R41)

Seed: 2026-05-08 swarm of three Sonnet reviewers — semi-formal-code-review, data-intensive-systems, platform-strategist. Validated 2026-05-08 by code-scout (opus) read-only pass against actual sources; corrections folded in (R24, R31, R32, R36) and five new findings appended (R37–R41).

### HIGH — correctness

#### R21. Duplicate `budget exceeded (alloc)` diagnostics on breach `[done]`

`src/evaluator/core.rs:396-400` (`track_alloc`) emits the diag when `alloc_limit_breached()` fires; the **next** `eval()` then hits `check_budget()` which re-checks alloc and emits again. Repeats every recursion.

**Fix:** added `budget_breached: bool` flag on `Evaluator`; first emission sets, subsequent budget diags suppressed. Added budget-test that asserts diagnostic count = 1 for breach scenarios.

#### R22. `BinaryOp::Concat` bypasses alloc budget entirely `[done]`

`src/evaluator/core.rs:1146-1157` produced `format!("{a}{b}")` without `track_alloc`. Every other heap-producing node (string literal, array, object, let) tracked.

**Fix:** `track_alloc((a.len() + b.len()) as u64)` before format!; early-return on breach.

#### R32. `ExtensionRegistry` results bypass alloc budget categorically `[done]`

`src/evaluator/core.rs:1469-1479`: `registry.call(name, &evaluated_args)` returned a `Value` never run through `track_alloc`.

**Fix:** post-charge `track_alloc(value_size_estimate(&result))` immediately after `registry.call` returns; on breach, replace with `Value::Null` + `budget exceeded (extension result)` diag. Defined `value_size_estimate` in `src/types.rs`.

### HIGH — cross-runtime integrity

#### R23. TS/WASM differential oracle missing `[done]`

`tests/differential_oracle.rs` covered Rust↔Python only. No TS/WASM oracle existed.

**Fix:** created `scripts/fel-wasm-eval.mjs` (Node stdin-to-JSON WASM evaluator), added `rust_wasm_parity` proptest, `wasm_val()` harness. `make test-differential` runs both Python and TS oracles.

#### R24. `power()` fractional/negative exponent falls to f64 — not in oracle corpus `[done]`

Integer exponent uses a hand-written `checked_mul` loop; fractional or negative exponent falls to `base_f.powf(exp_f)` (f64). Most likely divergence point.

**Fix:** seeded the oracle corpus with 10 hand-picked fractional/negative power cases. Added `power_fractional_negative_does_not_panic`, `power_fractional_negative_is_deterministic`, and `power_fractional_negative_rust_python_parity` tests.

### MEDIUM — coverage debt

#### R25. Phantom proptests in `tests/fel_proptest.rs` `[done]`

Three tests used `_ in any::<u8>()` and asserted constant string-literal expressions.

**Fix:** replaced with meaningful generators via `arb_value` / `arb_expr`.

#### R26. `parse_print_identity` skips most of `arb_expr` `[done]`

Five escape clauses (`.`, `let`, `if`, `then`, `[`) returned `Ok(())` unconditionally.

**Fix:** addressed the printer's asymmetric cases until all escape clauses could be removed. Each removal was one TDD cycle: reproduce the round-trip failure, fix the printer, drop the escape.

#### R27. Fuzz targets call unlimited `evaluate()` `[done]`

**Fix:** created `fuzz/fuzz_targets/fel_budget.rs` — calls `evaluate_with_budget` (later migrated to `evaluate_with` + `EvaluatorOptions`) with constrained budget.

#### R28. Differential oracle swallows Python failures `[done]`

`python_val()` returned `None` on any non-zero exit or non-JSON output.

**Fix:** distinguish parse errors (skip) from panics (fail). Capture stderr; pattern-match against expected parse-error prose.

#### R29. `arb_value` generates single-element arrays only `[done]`

**Fix:** `prop::collection::vec(arb_value(depth-1), 0..4).prop_map(Value::Array)`.

#### R30. Fuzz crash artifacts not replayed in CI `[done]`

**Fix:** CI step added to `.github/workflows/doc.yml` for fuzz artifact replay.

### MEDIUM — design / API surface

#### R31. `EvalBudget` API hygiene `[done]`

**Fix:** added `EvalBudget::for_batch(steps, alloc)` and `EvalBudget::for_interactive(deadline)` constructors. Doc-comment on `deadline` field.

### MEDIUM — platform positioning

#### R33. No semver / stability commitment, not on crates.io `[done]`

**Fix:** added "Versioning posture" section to README — pre-1.0, no crates.io, path-coupled only. Chose option B (hold 1.0) over A (publish 0.x).

#### R34. No external FEL semantics spec doc `[done]`

**Fix:** created `docs/SPEC.md` — 255-line FEL semantics specification covering grammar, evaluation rules, builtin catalog, budget contract, diagnostic wire shape.

#### R35. External conformance corpus `[done]`

**Fix:** created `conformance/` directory with README + `fel-conformance.jsonl` (249 fixtures). Added `make conformance` target and `src/bin/emit-conformance-fixtures.rs`.

#### R36. Diagnostic taxonomy partial; calendar internals leaked `[done]`

**Fix:** 
- Added README Diagnostics section with full closed taxonomy, stability commitment.
- Moved `civil_from_days`/`days_from_civil`/`days_in_month` to `pub(crate)` in `src/types.rs`.

### New observations from opus validation pass (R37–R41)

#### R37. `make_string` / value-constructor seam — prevent future Concat-style omissions `[done]`

**Fix:** introduced `Evaluator::make_{string,array,object}()` helpers; migrated all heap-producing eval sites; removed manual `track_alloc` calls at construction sites.

#### R38. `EvalBudget::tiny()` misnamed `[done]`

**Fix:** renamed to `EvalBudget::min_viable()`.

#### R39. `MAX_STRATEGY_DEPTH` constant is dead `[done]`

**Fix:** wired as the default depth parameter, removed `#[allow(dead_code)]`.

#### R40. `evaluate*` API surface is parameter-object smell `[done]`

**Fix:** introduced `EvaluatorOptions { trace, extensions, budget }` with `Default::default()`. Collapsed 8 entry points to `evaluate()` and `evaluate_with()`. Deprecated old functions.

#### R41. `Concat` is the worst untracked heap-grower but other format!-bearing paths exist `[done]`

**Fix:** closed as no-op once R37 landed. The value-vs-diagnostic distinction via `make_string` helper made this a non-issue.

### Post-review fixes (2026-05-08)

The semi-formal code review identified three findings that were fixed after the main implementation:

- **Finding 2 (WARNING):** Builtin string functions (`upper`, `lower`, `trim`, `replace`, `format`, `substring`, `string()`, `typeOf`, `moneyCurrency`) bypassed alloc budget. Fixed: `fn_str1` signature changed from `fn(&str) -> Value` to `fn(&str) -> String` + wraps through `make_string`; all direct `Value::String(...)` sites in builtin files routed through `make_string`. `make_string` visibility widened to `pub(in crate::evaluator)`.
- **Finding 5 (NIT):** `fel_budget` fuzz target used deprecated `evaluate_with_budget`. Fixed: switched to `evaluate_with` + `EvaluatorOptions`.
- **Finding 9 (NIT):** Power oracle no-panic test had vacuous diagnostic assertion. Fixed: removed the vacuous `!message.contains("panic")` assertion (test value is that it completes without panicking).

All 575 tests pass across 23 test targets.

---

## 2026-05-17 internal-ratification pass

Prepared FEL as a W3C-style internally ratified specification/tool surface:

- Added explicit status, conformance classes, normative references,
  versioning, security, privacy, and internationalization sections to
  `docs/SPEC.md`.
- Added `conformance/manifest.json` with the public corpus line count and
  SHA-256 digest.
- Added `scripts/check-ratification.py` plus `make check-ratification` and
  `make ratify` gates. The local gate validates required spec headings,
  conformance JSONL schema, manifest integrity, public corpus regeneration,
  all-features tests, and rustdoc broken-link denial.
- Removed stale `tests/fixtures/conformance.jsonl`, which used an obsolete
  `{source,value}` shape and was not consumed by tests. The public corpus in
  `conformance/fel-conformance.jsonl` is now the single ratification corpus.
- Documented `make ratify-external` as the cross-runtime implementation-report
  gate for sibling Python and WASM runtimes.
- Added normative public/result JSON versus typed wire JSON language. Native
  JavaScript `BigInt` is explicitly not a wire type; exact FEL number
  interchange uses tagged JSON with decimal strings for fractional decimals and
  unsafe integers.
- Tightened `fel_to_json` / `fel_to_ui_json` number emission so unsafe integers
  and decimal values that would stringify imprecisely fall back to decimal
  strings. Tightened `fel_to_wire_json` so tagged number values use strings for
  unsafe integers as well.
- Updated the WASM differential helper to compare raw JSON returned from WASM,
  avoiding Node-side precision loss before the Rust oracle can compare values.
  Added saved proptest regression seeds for the large-integer and tiny-decimal
  Rust↔WASM failures.
- Verified `make ratify` and `make ratify-external` on 2026-05-17. The external
  gate required rebuilding the ignored sibling `formspec-engine` runtime WASM
  artifact so it reflected this checkout's `fel-core`.
