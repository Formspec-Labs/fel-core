# fel-core — backlog status

Audit seed: 2026-05-06 multi-agent review.

Audit backlog items are **closed** and summarized in [`COMPLETED.md`](COMPLETED.md) (including the **Delivered ID** quick reference).

**Open follow-ups (R1–R20) from the 2026-05-06 semi-formal review** are **complete** as of 2026-05-06 — see section **"Open backlog R1–R20"** in [`COMPLETED.md`](COMPLETED.md).

---

# Chaos / pressure / edge-case initiative (C1–C10)

Seed: 2026-05-07. Frame = harden the foundational expression-language substrate against semantic regression, cross-runtime divergence, and resource exhaustion before downstream consumers (formspec engine TS, formspec-py, wos-server, case-portal) calcify around the current behavior.

**Architectural prerequisite chain** (sequence is design-driven, not calendar-driven):

```
C4 (EvalBudget seam) → C1 (AST proptest strategies) → C2, C3, C5 (properties built on C1)
C6, C7 (fuzz pipeline)         independent
C8 (coverage audit)            run last; validates prior coverage
C9, C10 (cheap insurance)      independent
```

C4 leads because the budget parameter threads through the `evaluate*` signatures (`src/evaluator/core.rs:214`, `:232`, `:261`, `:283`); retrofitting after downstream callers calcify is the kind of architectural debt the economic model penalizes most.

---

## HIGH

### C1. AST-generative proptest strategies `[done]`

Imp: `src/testing/strategies.rs` behind `cfg(any(test, feature = "proptest-strategies"))`. `arb_value`, `arb_expr`, `arb_decimal` composed for structural shrinking. Tests in `tests/ast_proptest.rs`.

`tests/parser_parse_proptest.rs:14` emits random UTF-8 — most strings are noise, shrinking is meaningless. `tests/fel_proptest.rs:24-30` round-trips only `i32` integer literals. Property coverage is shallow because the input distribution is byte-level, not AST-level.

**Architectural seam:** `src/testing/strategies.rs` behind `cfg(any(test, feature = "proptest-strategies"))`. Public surface:

- `arb_value(depth: u32) -> impl Strategy<Value = Value>` — covers all `Value` variants (Null, Bool, Number, String, Array, Object, Money, Date, etc.).
- `arb_expr(depth: u32, catalog: &BuiltinCatalog) -> impl Strategy<Value = Expr>` — well-typed AST: literals, calls bound to the live builtin catalog, arrays/objects, field paths, all binary/unary ops, `if`. Sub-strategies compose so proptest derives structural shrinking for free.
- `arb_decimal()` — biased to overflow-adjacent values, sub-precision values, and zero/one identities.

**Properties this row delivers** (`tests/ast_proptest.rs`):

1. `parse(print(ast)) == ast` — printer/parser fixpoint over full AST (today: integers only).
2. `eval(parse(s), env) == eval(parse(s), env)` — determinism across two evaluations.
3. No panic on `tokenize/parse/print/eval` for any generated AST.
4. Every `Decimal` operation in eval returns `Ok(_)` or a typed error — never panics, never produces a poisoned NaN-equivalent. Locks down commit `76673bd`.

**Done when:** strategies module exists, four properties green at `cases = 256`, shrinking demonstrably reduces a planted bug to a minimal AST.

### C2. Semantic invariant property table `[done]`

### C3. Cross-runtime differential oracle `[done]`

### C4. Resource-budget enforcement (`EvalBudget` seam) `[done]`

### C5. Decimal property coverage extension `[done]`

### C6. Fuzz-to-regression pipeline `[done]`

### C7. Conformance corpus as fuzz seed + dictionary `[done]`

### C8. Coverage-guided gap audit `[done]`

### C9. Concurrency smoke `[done]`

### C10. Snapshot error messages `[done]`

`tests/parser_rejection_tests.rs` and evaluator diagnostic paths produce error prose that nothing currently pins. Wording drifts silently across refactors; downstream consumers parsing message text break invisibly.

**Deliverable:** add `insta` as dev-dep; snapshot every error message produced by the rejection suite and the evaluator diagnostic suite. PR diffs surface message changes for explicit review.

---

## Execution protocol (unchanged)

For future behavioral changes:

1. **Red** — Test fails and proves the gap.
2. **Green** — Smallest fix.
3. **Refactor** — Cleanup, same behavior.
4. **Verify** — Focused tests, then full fel-core suite.

Guardrails: one behavioral change per cycle; never ship without a test that demonstrated the bug or regression risk.
