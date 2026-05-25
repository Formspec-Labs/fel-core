# Phase 3 CI gate — design

**Date:** 2026-05-23
**Status:** DESIGN (no implementation)
**Branch base:** `main` @ `75653b9`
**Companion:** [`2026-05-23-test-suite-triage.md`](./2026-05-23-test-suite-triage.md) §"Phase 3 — Leverage policy in CI"

## Motivation

Phase 2 measured what example-based tests miss. `extract_dependencies` sat at **56 %** kill rate with only example tests (`environment_integration_tests.rs`) pinning it; FUT-5 added one proptest file (`dependencies_proptest.rs`) and the same surface jumped to **100 %**. `prepare_host` showed the same shape (69 % → uplifted by FUT-4). Example tests document *intent*; property tests pin *contract*. The Phase 3 gate codifies that asymmetry as a normative discipline: a public re-export in `src/lib.rs` does not count as "covered" until ≥1 property test exercises it. Example tests remain valuable as executable documentation, but they cannot be the only thing standing between a re-export and a regression. This is not a coverage-percentage gate — it is a structural gate on the **public-API → proptest** mapping.

**Proxy framing.** The gate uses re-export presence as a tractable proxy for behavioral contract — easy to grep, easy to enumerate, easy to fail-loudly on drift. It is an imperfect proxy: some re-exports are pure data carriers, some are tag enums, some are third-party. The exemption categories (E1–E8 below) exist precisely because of that imperfection. Where the proxy under-counts (a non-re-exported internal function with rich behavior), Phase 2 mutation testing is the safety net. Where the proxy over-counts (a tag enum with no behavior of its own), exemptions opt out with reviewer-readable justification. The gate is the structural skeleton; mutation testing is the semantic muscle. Both are required.

## Current state

Snapshot of `src/lib.rs` re-exports (sha `75653b9`) → proptest coverage. "Direct" = the proptest names or constructs the symbol explicitly. "Transitive (arb_expr)" = exercised because `arb_expr` generates AST nodes that flow through the symbol when parsed/evaluated/printed. "None" = no proptest references it; only example tests (or no test).

**Audit provenance.** Table re-audited at sha `9d44f38`; every "Transitive" or "Indirect" claim is verified by `grep -rn '<symbol>' tests/` against actual usage rather than transitive reasoning alone. Canonical re-export paths follow `src/lib.rs` (e.g. `Environment` is re-exported under `evaluator::`, not `environment::` — the rows below reflect the actual path).

### Module re-exports (`pub mod`)

These are public modules; the gate applies to the named symbols re-exported from them via `pub use ...::{...}`, not to the modules themselves. `pub mod ast`, `convert`, `dependencies`, `environment`, `error`, `evaluator`, `extensions`, `lexer`, `parser`, `prepare_host`, `printer`, `types`, `extensions`, and the `testing` module (gated `cfg(any(test, feature = "proptest-strategies"))`) are out-of-scope as a unit — covered via their re-exports below.

### Symbol re-exports (`pub use`)

| Re-export | Direct proptest | Transitive (`arb_expr`) | Status |
|---|---|---|---|
| `ast::Expr` | `ast_proptest.rs`, `fel_proptest.rs`, `dependencies_proptest.rs` | yes | **COVERED** |
| `context_json::formspec_environment_from_json_map` | — | — | **GAP** |
| `convert::fel_to_json` | `fel_proptest.rs:json_scalar_roundtrip` | yes | **COVERED** |
| `convert::fel_to_ui_json` | — | — | **GAP** |
| `convert::fel_to_wire_json` | — | — | **GAP** |
| `convert::field_map_from_json_str` | — | — | **GAP** |
| `convert::json_object_to_field_map` | — | — | **GAP** (used in `public_conformance_corpus.rs` example tests only) |
| `convert::json_to_fel` | `fel_proptest.rs:json_object_order_roundtrip`, `json_scalar_roundtrip` | yes | **COVERED** |
| `dependencies::Dependencies` | `dependencies_proptest.rs:*` | yes | **COVERED** |
| `dependencies::dependencies_to_json_value` | `dependencies_proptest.rs:dependencies_to_json_value_reflects_actual_deps` (example, in proptest file) | — | **MIXED** — see §Mechanism |
| `dependencies::dependencies_to_json_value_styled` | — | — | **GAP** |
| `dependencies::extract_dependencies` | `dependencies_proptest.rs:extract_is_idempotent`, `fields_union_under_binary_add`, `wildcard_propagates_through_binary_op` | yes | **COVERED** (Phase 2 lifted 56 % → 100 %) |
| `environment::FormspecEnvironment` | — | — | **GAP** (example-only in `env_mip_tests.rs`, `locale_fel_functions.rs`) |
| `environment::MipState` | — | constructed via struct-literal in `env_mip_tests.rs` (example) | **EXEMPT?** — data-shape struct; see §Exemption categories |
| `environment::RepeatContext` | — | — | **EXEMPT?** — data-shape struct |
| `error::Diagnostic` | — | indirect (eval results carry diagnostics; no proptest asserts on them) | **GAP** |
| `error::DiagnosticKind` | — | indirect | **GAP** (P0 per triage; example-only) |
| `error::Error` | — | indirect | **EXEMPT?** — error enum returned by `Result`-bearing APIs whose `Ok` path is property-tested |
| `error::ParseError` | — | indirect | **EXEMPT?** — same shape as `Error` |
| `error::Severity` | — | — | **EXEMPT?** — enum tag |
| `error::fel_diagnostics_to_json_value` | — | — | **GAP** |
| `error::fel_diagnostics_to_json_value_styled` | — | — | **GAP** |
| `error::has_error_diagnostics` | example in `evaluator_tests.rs:has_error_diagnostics_discriminates_severity` | — | **GAP** |
| `error::reject_undefined_functions` | — | — | **GAP** |
| `error::undefined_function_names_from_diagnostics` | example in `evaluator_tests.rs` | — | **GAP** |
| `evaluator::BudgetExceededKind` | — | — | **EXEMPT?** — enum tag returned by `EvalBudget::check`; `EvalBudget` itself proptest-eligible (none yet — **GAP**) |
| `evaluator::ContextBinding` | example in `host_bindings.rs` | — | **GAP** |
| `evaluator::ContextBindingCatalog` | trait, impls in `host_bindings.rs` examples | — | **EXEMPT?** — trait surface |
| `evaluator::ContextBindingKind` | example | — | **EXEMPT?** — enum tag |
| `evaluator::EmptyCatalog` | example | — | **EXEMPT?** — ZST sentinel |
| `evaluator::Environment` | trait surface; impls in `host_bindings.rs:58,357` and `trace_tests.rs:60` are example tests only | — | **EXEMPT?** — trait surface (E6); canonical impl is `MapEnvironment` |
| `evaluator::EvalBudget` | example in `budget_tests.rs` | — | **GAP** (P0) |
| `evaluator::EvalResult` | result type of `evaluate`; carried through all eval proptests | yes | **COVERED** |
| `evaluator::Evaluator` | — | indirect | **EXEMPT?** — internal driver type; `evaluate` is the public entry |
| `evaluator::EvaluatorOptions` | example | — | **GAP** |
| `evaluator::MapEnvironment` | `semantic_invariants.rs:*`, `fel_proptest.rs:*`, `ast_proptest.rs:*`, `differential_oracle.rs:18` (constructed in `rust_val`), `fel_chaos_proptest.rs:tokenize_parse_eval_do_not_panic` | yes | **COVERED** |
| `evaluator::UNBOUND_CONTEXT_REF_CODE` | example in `host_bindings.rs` | — | **EXEMPT?** — error-code constant |
| `evaluator::eval_with_fields` | example in `evaluator_tests.rs` | — | **GAP** |
| `evaluator::evaluate` | every eval proptest | yes | **COVERED** |
| `evaluator::evaluate_with` | example in `trace_tests.rs` | — | **GAP** |
| `evaluator::evaluate_with_catalog` | example in `host_bindings.rs` | — | **GAP** |
| `extensions::ExtensionCallOutcome` | — | flows through `ExtensionRegistry::call` (P0 GAP); not named in any test | **EXEMPT?** — enum tag; consumer (`ExtensionRegistry`) is P0 GAP — exemption blocked until consumer covered |
| `extensions::ExtensionError` | — | flows through `ExtensionRegistry::call`; not named in any test | **EXEMPT?** — error enum; same caveat as `ExtensionCallOutcome` |
| `extensions::ExtensionFn` | — | — | **EXEMPT?** — function-pointer typedef |
| `extensions::ExtensionFunc` | — | — | **EXEMPT?** — struct holding `ExtensionFn` |
| `extensions::ExtensionRegistry` | example in `budget_tests.rs` | indirect | **GAP** (P0) |
| `extensions::Package` | example | — | **EXEMPT?** — registry-grouping struct |
| `extensions::builtin_function_catalog` | every proptest using `arb_expr(_, catalog)` | yes | **COVERED** |
| `extensions::builtin_function_catalog_for` | — | — | **GAP** |
| `extensions::builtin_function_catalog_json_value` | — | — | **GAP** |
| `extensions::builtin_function_catalog_json_value_for` | — | — | **GAP** |
| `indexmap::IndexMap` | — | — | **EXEMPT** — third-party crate re-export, not our contract |
| `interpolation::expr_is_interpolation_static_literal` | — | — | **GAP** (example test in `interpolation_static_literal.rs`) |
| `iso_duration::IsoDurationParse` | — | — | **EXEMPT?** — enum returned by parser; its parsers below are the testable surface |
| `iso_duration::parse_iso8601_duration` | — | — | **GAP** (Tier-2 in mutation gate) |
| `iso_duration::parse_iso8601_duration_ms` | — | — | **GAP** (Tier-2) |
| `lexer::PositionedToken` | — | indirect | **EXEMPT?** — struct returned by `tokenize` |
| `lexer::is_valid_fel_identifier` | — | — | **GAP** |
| `lexer::sanitize_fel_identifier` | — | — | **GAP** |
| `lexer::tokenize` | `ast_proptest.rs`, `fel_chaos_proptest.rs` | yes | **COVERED** |
| `lexer::tokenize_to_json_value` | example in `lexer_tests.rs` (kill test) | — | **GAP** |
| `lexer::tokenize_to_json_value_styled` | — | — | **GAP** |
| `parser::parse` | every parse/eval proptest | yes | **COVERED** (P0) |
| `prepare_host::PrepareHostInput` | `prepare_host_proptest.rs:*` | yes | **COVERED** |
| `prepare_host::PrepareHostOptions` | — | indirect | **EXEMPT?** — options struct fed into proptest-covered `prepare_for_host` |
| `prepare_host::host_options_from_json` | — | — | **GAP** |
| `prepare_host::prepare` | — | — | **GAP** (P0 — sibling of `prepare_for_host`) |
| `prepare_host::prepare_for_host` | `prepare_host_proptest.rs:*` (Phase 2 FUT-4) | yes | **COVERED** |
| `printer::print_expr` | `ast_proptest.rs:parse_print_identity`, `fel_proptest.rs:parse_print_roundtrip_decimal_integer`, `differential_oracle.rs` | yes | **COVERED** |
| `trace::Trace` | example in `trace_tests.rs`, `evaluator_tests.rs` | — | **GAP** |
| `trace::TraceStep` | example | — | **EXEMPT?** — enum returned via `Trace` |
| `types::CurrencyCode` | example in `evaluator_tests.rs` | — | **EXEMPT?** — newtype around `&str` parsed via `parse` |
| `types::Date` | `arb_value` covers; `semantic_invariants.rs`, `fel_proptest.rs` | yes | **COVERED** |
| `types::Money` | `arb_value` covers via `fel_proptest.rs` | yes | **COVERED** |
| `types::Value` | every eval proptest | yes | **COVERED** |
| `types::parse_date_literal` | — | indirect via `arb_expr` `DateLiteral` round-trip | **GAP** |
| `types::parse_datetime_literal` | — | indirect | **GAP** |
| `types::value_size_estimate` | — | — | **GAP** |
| `wire_style::JsonWireStyle` | — | — | **EXEMPT?** — enum tag consumed by `*_styled` siblings |

## Gap

Sorted by Phase-2 P0 priority (highest leverage first):

**P0 — must close before gate turns on:**
1. `evaluator::EvalBudget` — P0 seam per triage; only example tests in `budget_tests.rs`.
2. `extensions::ExtensionRegistry` — P0 seam; only example tests.
3. `error::DiagnosticKind` + `error::Diagnostic` — closed/append-only public API per triage; no proptest pins the shape.
4. `prepare_host::prepare` — P0 sibling of `prepare_for_host`; the two functions diverge on output shape only and one proptest can cover both.
5. `convert::fel_to_ui_json` + `convert::fel_to_wire_json` — wire-format with TS+Python; same shape as `fel_to_json` which is covered. One proptest with a `JsonWireStyle` parameter closes both.

**Tier-2:**
6. `iso_duration::parse_iso8601_duration{,_ms}` — Tier-2 in mutation gate; pure parser with non-trivial edge logic. One proptest on round-trip (`duration → format → parse`) closes both.
7. `interpolation::expr_is_interpolation_static_literal` — pure AST predicate; one proptest checking the predicate is closed under generated `arb_expr` inputs.
8. `evaluator::evaluate_with` + `evaluator::evaluate_with_catalog` + `evaluator::eval_with_fields` — three siblings of `evaluate`. One proptest with a parametric driver covers all three.
9. `trace::Trace` — non-trivial mutable accumulator; proptest on monotonicity (length never decreases across `evaluate_with` calls) is a real invariant.
10. `lexer::tokenize_to_json_value{,_styled}` + `error::fel_diagnostics_to_json_value{,_styled}` + `dependencies::dependencies_to_json_value{,_styled}` — six JSON-export siblings. One proptest per pair, parameterized by `JsonWireStyle`.

**Convenience surface (lower priority):**
11. `lexer::is_valid_fel_identifier` + `lexer::sanitize_fel_identifier` — round-trip property: `sanitize(s) ⇒ is_valid(sanitize(s))`.
12. `error::reject_undefined_functions` + `error::undefined_function_names_from_diagnostics` + `error::has_error_diagnostics` — diagnostic-filter helpers; one proptest on a generated `Vec<Diagnostic>` covers the trio.
13. `types::parse_date_literal` + `types::parse_datetime_literal` + `types::value_size_estimate` + `context_json::formspec_environment_from_json_map` + `extensions::builtin_function_catalog_for` + `extensions::builtin_function_catalog_json_value{,_for}` + `convert::field_map_from_json_str` + `convert::json_object_to_field_map` + `prepare_host::host_options_from_json` — JSON-input parsers and catalog-export helpers.

Total: **~16 proptests** close ~30 gaps. Initial estimate was ~13 assuming `_styled` JSON-export siblings collapse cleanly under a `JsonWireStyle` parameter; on closer read each `_styled` sibling differs in output *shape* (not just an output flag) — at minimum the proptest body needs a `match style { Compact => ..., Styled => ... }` branch with separate invariant assertions, and several pairs warrant separate property statements rather than a parameterized one. Phase 3a closeout will report the actual count.

## Exemption categories

The gate as literally stated ("each `pub use` requires ≥1 proptest") would mandate proptests for symbols where a property test is not the right artifact. Per the prompt's standing authorization to name exemption categories, these are out of scope:

- **(E1) Third-party re-exports.** `indexmap::IndexMap`. Not our contract; upstream owns the property surface.
- **(E2) Re-export-only legibility.** Reserved for the case where the same symbol is re-exported under two paths for namespace ergonomics. Not currently used — `Environment` and `MapEnvironment` are only re-exported under `evaluator::`, not duplicated. Kept as a category for the foreseeable case of a future cross-module alias.
- **(E3) Plain data carriers with no behavior.** `MipState`, `RepeatContext`, `ContextBinding`, `PositionedToken`, `TraceStep` — structs/enums whose construction is mechanical and whose use is via methods on owning types. The property surface lives on the *consumer*. **Manifest MUST cite the consumer's proptest** (`consumer_proptests = ["tests/foo.rs::bar"]`) and the gate verifies the citation resolves to a real test function. Without a verified consumer, the exemption is rejected.
- **(E4) Tag enums.** `Severity`, `BudgetExceededKind`, `ContextBindingKind`, `JsonWireStyle`, `ExtensionCallOutcome`. The behavior under each tag is what's testable; the tag itself is a discriminator. A proptest over the *consumer* of the tag (e.g. `EvalBudget::check` for `BudgetExceededKind`) is the right pin. **Same citation rule as E3** — `consumer_proptests` required and gate-verified.
- **(E5) Error/result enums.** `Error`, `ParseError`, `ExtensionError`. Exempt iff the `Ok` branch is property-tested AND the `Err` branch is example-tested. The "AND" is unverifiable from a status flag alone, so the manifest MUST carry both: `consumer_proptests = ["tests/foo.rs::ok_branch_property"]` for the Ok path AND `example_tests = ["tests/bar.rs:L42-L60"]` for the Err path. The gate verifies both citations resolve (grep on the file:line range, grep on the fn name). Failing either rejects the exemption. (`DiagnosticKind` is *not* exempt — it is the closed enumeration consumed by tooling, not a `Result::Err` shape.)
- **(E6) Trait surfaces.** `Environment`, `ContextBindingCatalog`. The trait itself is a port; the gate applies to a *canonical implementation* (e.g. `MapEnvironment`, `EmptyCatalog`).
- **(E7) Function-pointer typedefs and ZSTs.** `ExtensionFn`, `ExtensionFunc`, `EmptyCatalog`, `Package`. No behavior to property-test; consumer surface (`ExtensionRegistry`) is the pin.
- **(E8) Constants.** `UNBOUND_CONTEXT_REF_CODE`. A literal string; the right test is a snapshot, not a property.

Every exemption requires a one-line justification in the manifest. **Default is "not exempt"**; exemption is opt-in with reviewer sign-off.

## Mechanism design

Three options considered:

### Option A — Static manifest + Rust integration test

A `tests/lib_reexport_coverage_gate.rs` test loads a checked-in `tests/lib_reexport_coverage.toml` (or `.rs` const) that maps each `lib.rs` symbol to one of: `{ proptest = ["file::fn_name", ...] }`, `{ exempt = "category", reason = "..." }`. The test:

1. Parses `src/lib.rs` (regex or `syn`) and enumerates every `pub use` symbol and `pub fn`.
2. Asserts every symbol has a manifest entry.
3. Asserts every `proptest = [...]` entry resolves to a real file/function (grep-based).
4. Asserts every `exempt` entry cites one of the E1–E8 categories.

**Pros:** Lives in `cargo test`; fails CI on `cargo test` like any other test; reviewer-readable manifest; exemptions explicit.
**Cons:** Manifest is hand-maintained; drift is possible if a contributor edits `lib.rs` without updating the manifest (caught by the test, which is the point).

### Option B — Doc-comment convention

Every `pub use` in `lib.rs` requires an adjacent `///` doc comment of a fixed shape:

```rust
/// Proptest: tests/dependencies_proptest.rs::extract_is_idempotent
pub use dependencies::extract_dependencies;
```

A test parses `lib.rs` and asserts the convention.

**Pros:** Single source of truth (`lib.rs`); no parallel manifest.
**Cons:** `lib.rs` is already at the readability ceiling for a top-level module file; adding 60+ structured doc comments is noise; exemption category encoding gets awkward (`/// Exempt: E1 (third-party)`); harder to scan than a manifest table.

### Option C — Shell/Python CI script

A script greps `lib.rs` for `pub use`, greps `tests/` for `proptest!` blocks, and produces a report.

**Pros:** No Rust integration; fastest to write.
**Cons:** No exemption mechanism without a parallel file (collapses into Option A); not part of `cargo test` so easy to skip locally; brittle regex on `lib.rs`.

### Recommendation

**Option A.** It bakes the gate into `cargo test`, makes exemptions explicit and reviewable, and the manifest doubles as the audit artifact the Phase-2 baseline now provides for mutation. Option B fails the readability test on `lib.rs`. Option C is Option A without the integration test, which buys nothing.

Concrete shape:

```toml
# tests/lib_reexport_coverage.toml
[symbols."dependencies::extract_dependencies"]
proptests = ["tests/dependencies_proptest.rs::extract_is_idempotent"]
notes = "P0 seam; Phase 2 FUT-5 lifted kill rate 56% → 100%"

[symbols."indexmap::IndexMap"]
exempt = "E1"
reason = "Third-party re-export; upstream owns the property surface"

[symbols."evaluator::EvalBudget"]
proptests = []  # GAP — must be filled before gate turns on
gap_ticket = "FUT-7"

# E3/E4 — consumer citation required and gate-verified
[symbols."environment::MipState"]
exempt = "E3"
reason = "Plain data carrier; behavior lives on consumer"
consumer_proptests = ["tests/env_mip_tests.rs::mip_relevant_query_reflects_state"]

# E5 — both consumer proptest AND example test required
[symbols."error::Error"]
exempt = "E5"
reason = "Result::Err shape for parse; Ok branch property-tested, Err branch example-tested"
consumer_proptests = ["tests/fel_proptest.rs::parse_print_roundtrip_decimal_integer"]
example_tests = ["tests/parser_rejection_tests.rs:L225-L265"]
```

The test fails CI if:
- A `pub use` symbol in `lib.rs` has no manifest entry.
- A manifest entry has `proptests = []` and no `exempt` field.
- A `proptests = [...]` entry references a function that doesn't exist.
- An `exempt` value isn't one of E1–E8.
- An `exempt = "E3"`/`"E4"`/`"E5"` entry lacks `consumer_proptests = [...]`, or the cited proptest function doesn't exist.
- An `exempt = "E5"` entry lacks `example_tests = ["file.rs:Lstart-Lend"]`, or the file/line range doesn't resolve.

The test does **not** verify the proptest actually exercises the symbol semantically — that's reviewer judgment, encoded in the `notes` field. The gate is structural, not semantic. (Mutation testing in Phase 2 is the semantic gate.)

## Implementation plan

**Phase 3a — close the P0 gaps (no gate yet).** 5 proptest files added or extended:

1. `tests/eval_budget_proptest.rs` — `EvalBudget::check` monotonicity, `BudgetExceededKind` discrimination.
2. `tests/extension_registry_proptest.rs` — registry lookup is closed under name set; `Package` re-registration is idempotent.
3. `tests/diagnostic_proptest.rs` — `DiagnosticKind` round-trip via `fel_diagnostics_to_json_value`; closes `Diagnostic`, `DiagnosticKind`, `has_error_diagnostics`, `reject_undefined_functions`, `undefined_function_names_from_diagnostics`.
4. Extend `tests/prepare_host_proptest.rs` — add `prepare` (non-host variant) alongside the existing `prepare_for_host` properties.
5. Extend `tests/fel_proptest.rs` — parameterize the JSON-roundtrip property over `JsonWireStyle`, closing `fel_to_ui_json` and `fel_to_wire_json`.

**Phase 3b — Tier-2 gap closures.** ~8 more proptest functions across existing files (round-trip on `parse_iso8601_duration`, predicate-closure on `expr_is_interpolation_static_literal`, monotonicity on `Trace`, etc.). Add to existing test files where possible; new file only when a new harness is needed.

**Phase 3c — write the manifest + gate test.** `tests/lib_reexport_coverage.toml` + `tests/lib_reexport_coverage_gate.rs`. First commit has the gate test `#[ignore]`'d so the manifest can land for review without blocking CI. Second commit removes the ignore.

**Commit message convention.** The activation commit (removes `#[ignore]`) MUST use the subject line `test(gate): activate lib_reexport_coverage_gate` so `git log --grep='activate lib_reexport_coverage_gate'` finds the activation point without scanning diffs.

**Phase 3d — wire into CI.** No new workflow needed; the gate runs as part of `cargo test` on the existing `ratification-gate` per-push job.

**Phase 3e — post-gate review.** Architecture review via `semi-formal-architecture-review` — confirms manifest categories, exemption justifications, and that no symbol slipped through as "covered" without a real proptest. Code review on the manifest itself.

**Owner-questions before starting:**
- **Q1.** Does the gate apply to `formspec-py` (PyO3 bindings) and `formspec-engine`/`formspec-wasm` (WASM bindings) consumers, or only to `fel-core`'s Rust public API? **Lean:** `fel-core` only for Phase 3. WASM/Py have their own conformance corpora (`make test-differential`) that pin behavior across the boundary; a separate gate for the bindings is a follow-up if needed.
- **Q2.** Does `#[cfg(any(test, feature = "proptest-strategies"))] pub mod testing` count as a public re-export? **Lean:** No. It is a test-helper surface, not a contract — exempt as E1-style (test-infrastructure, not consumer contract). Document explicitly in the manifest.
- **Q3.** Resolved as normative — see §Exemption categories E3/E4/E5. Manifest carries `consumer_proptests = [...]` (and `example_tests = [...]` for E5) and the gate verifies the citations resolve.

**Dependencies:** `cargo test` already runs every PR (no new CI workflow). One new dev-dep: add `toml = "0.8"` to `[dev-dependencies]` in `Cargo.toml`. Verified: no `toml` dep exists today (neither under `[dependencies]` nor `[dev-dependencies]`). A hand-rolled key-value parser was considered and rejected — it saves ~50 LOC versus a well-maintained, widely-used crate that already handles the manifest's escape/quote/nesting edge cases. Cost is one transitive dep tree in dev-build only.

## Failure mode

A contributor adds `pub use evaluator::NewThing;` to `lib.rs` and pushes. CI runs `cargo test`. The gate test (`lib_reexport_coverage_gate`) fails with:

```
FAIL: src/lib.rs declares `pub use evaluator::NewThing` but tests/lib_reexport_coverage.toml has no entry.

To resolve, choose ONE:

1. Add a proptest covering `NewThing` and add to the manifest:
   [symbols."evaluator::NewThing"]
   proptests = ["tests/new_thing_proptest.rs::roundtrip"]
   notes = "<one-line intent>"

2. If `NewThing` is a re-export-only legibility alias, plain data carrier,
   tag enum, or third-party re-export, exempt it:
   [symbols."evaluator::NewThing"]
   exempt = "E3"   # or E1, E2, E4, ...
   reason = "<one-line justification>"
   consumer_proptests = ["tests/foo.rs::bar"]   # REQUIRED for E3/E4/E5
   example_tests = ["tests/bar.rs:L42-L60"]     # REQUIRED for E5 (Err branch)

3. If `NewThing` should not be public, demote to `pub(crate)` and the
   gate stops applying.

See thoughts/2026-05-23-phase-3-ci-gate-design.md §Exemption categories.
```

The contributor's choices are: write the proptest, exempt with justification, or pull the symbol back inside the crate. No fourth path. The cost is upfront and small; the alternative (silent surface growth without property coverage) is the failure mode Phase 2 surfaced quantitatively (56 % kill rate on `extract_dependencies`).

**Lifecycle:**
- New re-export → manifest entry required.
- Removed re-export → manifest entry removed (gate fails on stale entry, also useful).
- Renamed re-export → manifest entry renamed; proptest references checked against the new symbol.
- Symbol moves between modules (e.g. a hypothetical `evaluator::Environment` → `environment::Environment` reshuffle) → manifest entry updated; the canonical path policy resolves duplicates.
- **Phase 2 mutation survivor on a manifested symbol** → tracked in `thoughts/followups.md` (or successor survivor backlog); does **not** fail the structural gate (the proptest exists, which is what the gate checks); the manifest entry's `notes` field MUST flag the known semantic gap with a survivor reference (e.g. `notes = "survivor: tests/X.rs::Y mutation Z survives — see followups.md#sym-name"`). Resolves once a strengthened proptest kills the mutant.

**What does not fail the gate:**
- Adding a new proptest for an already-covered symbol (manifest can list multiple).
- Refactoring a proptest body without changing its name.
- Adding example tests for any symbol (they don't displace the proptest requirement, they supplement).

## Open questions

1. **Granularity of "covers."** The manifest links symbol → proptest function. But a proptest like `extract_is_idempotent` covers `extract_dependencies` strongly and `Dependencies` (the return type) only weakly. Should the manifest distinguish "primary" vs "incidental" coverage? **My lean:** No. Mutation testing (Phase 2) is the semantic check on whether the proptest *actually* exercises the symbol; the manifest is structural. But a third column (`coverage: primary | transitive`) would aid review.
2. **Manifest format — TOML vs Rust const.** TOML is reviewer-friendlier; a Rust const is parser-free and refactor-safe (typo in symbol name → compile error). **My lean:** TOML for the v1 (lower friction); revisit if drift becomes a problem.
3. **Should the gate also assert mutation kill-rate ≥ N for each P0 symbol?** That would couple Phase 2 and Phase 3, which is conceptually clean but operationally heavy (mutation runs weekly, not per-push). **My lean:** No — keep gates orthogonal. Phase 2 = semantic, Phase 3 = structural. Crossing them creates a slow per-push gate.
4. **Exemption category drift.** E1–E8 may grow. Who arbitrates? **My lean:** Architecture review at each Phase 3 closeout. New categories require a deviation note in this doc plus the manifest schema update.
5. **`#[cfg(test)]` proptests inline in `src/`.** Two exist (`src/types.rs:589`, `src/testing/strategies.rs:396`). The manifest should be able to reference inline proptests with a syntax like `src/types.rs::tests::decimal_arithmetic_does_not_panic`. **My lean:** Yes, support both `tests/*.rs::fn` and `src/**/*.rs::mod::fn` resolutions.
