# fel-core — backlog status

Audit seed: 2026-05-06 multi-agent review.

Audit backlog items are **closed** and summarized in [`COMPLETED.md`](COMPLETED.md) (including the **Delivered ID** quick reference). **Open** work below is from the **2026-05-06 semi-formal review swarm** (formspec-scout); each row is a follow-up task.

---

## Open tasks (semi-formal review swarm, 2026-05-06)

Guardrails: same as [Execution protocol](#execution-protocol-unchanged) — red test where behavior changes, smallest fix, verify with focused + full suite.

| ID | Priority | Task |
|----|----------|------|
| **#R1** | P2 | **Tracing / eager builtins:** Restore or replace the safety note for the eager-trace whitelist: after the trace arg-cache fix, document whether impure functions may ever be whitelisted and what invariants still apply. Touch [`src/evaluator/util.rs`](src/evaluator/util.rs) (and any builtin dispatch comments that list eager names). |
| **#R2** | P2 | **`sum()` mixed Money + Number arrays:** Today all-Money arrays error with guidance; mixed arrays sum only `Number` rows and skip Money silently. Decide product behavior (reject with diagnostic, coerce, or document), implement, and add a regression test. [`src/evaluator/builtins/aggregates.rs`](src/evaluator/builtins/aggregates.rs). |
| **#R3** | P3 | **Trace arg cache:** Document the contract that `CallArgCache` uses `args` slice identity (`*const Expr`) for the duration of `eval_function`, or refactor to a key that cannot go stale if the evaluator ever reallocates args mid-call. [`src/evaluator/core.rs`](src/evaluator/core.rs). |
| **#R4** | P3 | **ISO-8601 fractional seconds:** Document (README or `iso_duration` module) that fractional-second parsing uses ms derived from the first four fractional digits only; decide if deeper precision or full rational reconstruction is required for spec parity. [`src/iso_duration.rs`](src/iso_duration.rs). |
| **#R5** | P2 | **Chained equality:** Relational chains (`1 < 2 < 3`) are rejected; `parse_equality` still allows repeated `==` / `!=`. Align with spec intent—either reject chained equality in the parser with tests or document allowed grammar. [`src/parser.rs`](src/parser.rs). |
| **#R6** | P3 | **Parser recursion cap:** `max_recursion_depth` (32) rejects deeply nested parens. Document the limit in README/API notes and/or make it configurable if legitimate generated expressions need deeper nesting. [`src/parser.rs`](src/parser.rs). |
| **#R7** | P3 | **Predicate builtin arity:** Comments/tests say “exactly 2 arguments” but code uses `require_min_args` (minimum 2). Align messaging and tests with implementation, or switch to `require_exact_args` if extra arguments must be errors. [`src/evaluator/core.rs`](src/evaluator/core.rs), [`tests/evaluator_tests.rs`](tests/evaluator_tests.rs). |
| **#R8** | P3 | **Trace + extensions:** `evaluate_with_trace` uses `extensions: None`. Either document that traced evaluation cannot invoke extension fallbacks, add `evaluate_with_trace_and_extensions` (or equivalent), and cover with a test. [`src/evaluator/core.rs`](src/evaluator/core.rs). |
| **#R9** | P4 | **Public façade docs:** Explain in README why some capabilities are crate-root re-exports vs `fel_core::<module>` paths (e.g. no `fel_core::trace` module). [`README.md`](README.md). |
| **#R10** | P4 | **Optional API:** Consider `pub use indexmap::IndexMap` (or a type alias) at the crate root so hosts building `Value::Object` do not need a direct `indexmap` dependency—only if API stability goals warrant it. [`src/lib.rs`](src/lib.rs), [`src/types.rs`](src/types.rs). |
| **#R11** | P4 | **Interpolation static literal:** Add an explicit `Expr::VarRef { .. } => false` arm in `expr_is_interpolation_static_literal` for readability (no behavior change). [`src/interpolation.rs`](src/interpolation.rs). |
| **#R12** | P2 | **Test coverage:** Add one integration test that bare `VarRef` and equivalent `$` `FieldRef` resolve and evaluate the same for a controlled environment (parity beyond shared `eval_field_ref`). [`tests/`](tests/). |
| **#R13** | P4 | **CurrencyCode:** Document the panic contract of `CurrencyCode::as_str` (valid UTF-8 invariant after `parse`) if external construction is ever exposed. [`src/types.rs`](src/types.rs). |
| **#R14** | P2 | **Diagnostics consistency:** `Expr::Ternary` / `IfThenElse` non-boolean condition uses `"if: condition must be boolean, got …"` while `if()` builtin uses `reject_expected_type` (`"if: expected boolean, got …"`). Route the AST path through `reject_expected_type("if", "boolean", &cond)` or align strings. [`src/evaluator/core.rs`](src/evaluator/core.rs), [`src/evaluator/builtins/logic_types.rs`](src/evaluator/builtins/logic_types.rs). |
| **#R15** | P2 | **Structured diagnostics:** Either extend `DiagnosticKind` for type mismatches (e.g. expected vs actual type) so JSON consumers can classify errors, or explicitly document that type errors are message-only (no `kind`). [`src/error.rs`](src/error.rs), README if public contract. |
| **#R16** | P3 | **Tests:** Strengthen `test_undefined_function` (or add sibling test) to assert `DiagnosticKind::UndefinedFunction` and/or `fel_diagnostics_to_json_value` shape—not only non-empty diagnostics. [`tests/evaluator_tests.rs`](tests/evaluator_tests.rs), [`src/error.rs`](src/error.rs) tests as reference. |
| **#R17** | P4 | **`get_array` messaging:** Optional helper so `"expected array, got …"` matches the same pattern as `reject_expected_type` for copy/search consistency. [`src/evaluator/core.rs`](src/evaluator/core.rs). |
| **#R18** | P3 | **Silent date coercion:** `eval_date_operand` returns invalid dates as null without a dedicated diagnostic in some call paths—decide whether date builtins should emit a structured/type diagnostic on bad literals (may be spec-dependent). [`src/evaluator/builtins/dates.rs`](src/evaluator/builtins/dates.rs). |
| **#R19** | P4 | **Stress test:** `tokenize_long_expression` only asserts token count; add parse + evaluate cross-check (e.g. same string as `long_flat_addition_chain`) so wrong token kinds are caught. [`tests/stress_tests.rs`](tests/stress_tests.rs). |
| **#R20** | P4 | **Optional E2E:** One test that runs `formspec_environment_from_json_map` then `evaluate` on a representative WASM-shaped payload, complementing unit tests that stop at env construction. [`src/context_json.rs`](src/context_json.rs) `#[cfg(test)]` or [`tests/environment_integration_tests.rs`](tests/environment_integration_tests.rs). |

**Priority legend:** P2 = user-visible consistency or coverage gap; P3 = maintainability / tooling; P4 = polish or optional API/docs.

---

## Execution protocol (unchanged)

For future behavioral changes:

1. **Red** — Test fails and proves the gap.
2. **Green** — Smallest fix.
3. **Refactor** — Cleanup, same behavior.
4. **Verify** — Focused tests, then full fel-core suite.

Guardrails: one behavioral change per cycle; never ship without a test that demonstrated the bug or regression risk.
