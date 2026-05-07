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

### C1. AST-generative proptest strategies `[open]`

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

### C2. Semantic invariant property table `[open]`

`tests/fel_proptest.rs:34-46` checks four null-propagation cases on hardcoded sources. The algebra of FEL is otherwise unpinned — every future builtin can silently break commutativity, identity, or De Morgan with no signal.

**Deliverable:** `tests/semantic_invariants.rs`, one `proptest!` block per algebraic law, all consuming C1's `arb_value`/`arb_expr`. Dense table, not narrative tests:

- **Null propagation** — `op(null, x) == null` for every numeric / comparison op; whitelist `=`, `≠`, `coalesce`, `if`, `present`, `empty`. One unified property over the whitelist, not one test per op.
- **Commutativity** — `a + b == b + a`, `a * b == b * a`, `a = b == b = a`, `a ≠ b == b ≠ a`, `a and b == b and a`, `a or b == b or a`.
- **Associativity (decimal-precision-aware)** — `(a + b) + c == a + (b + c)` for non-overflowing inputs; `(a and b) and c == a and (b and c)`; same for `or`, string concat.
- **Identity** — `x + 0 == x`, `x * 1 == x`, `x and true == x`, `x or false == x`, `concat(x, "") == x`.
- **Idempotence** — `sort(sort(x)) == sort(x)`, `unique(unique(x)) == unique(x)`, `not(not(x)) == x`.
- **Conditional** — `if(true, x, y) == x`, `if(false, x, y) == y`, `if(c, x, x) == x`.
- **Aggregates** — `length(concat(a, b)) == length(a) + length(b)`, `count(arr) == arr.len()`, `sum([x]) == x`, `sum(arr) == reduce(+, 0, arr)`.
- **De Morgan** — `not(a and b) == (not a) or (not b)`, dual.

**Done when:** every law above has a green property; planting a violation in a builtin (e.g. swapping `+` to `-` in `concat`) causes the matching property to fail with a minimized counterexample.

### C3. Cross-runtime differential oracle `[open]`

Cross-runtime parity is a product requirement (FEL spec §11; consumers: formspec-engine TS, formspec-py, wos-server). Today there is no automated check; semantic divergence between Rust and Python evaluators is invisible until a downstream form behaves differently.

**Architectural seam:** fel-core is the canonical author. C1's `arb_expr` generates the input distribution; fel-core emits structured `(source, value_json)` fixtures consumed by sibling runtimes via the same conformance harness.

**Deliverable in fel-core scope:**

- `tests/differential_oracle.rs` — generates ASTs via C1, evaluates in Rust, shells out to `formspec/.venv/bin/python -m formspec.fel.eval --json` (or equivalent already exposed by `formspec-py`), compares `Value` JSON. `#[ignore]` by default; `make test-differential` enables it when the sibling venv is detected.
- `src/bin/emit-conformance-fixtures.rs` — replayable AOT generator writing `tests/fixtures/conformance/*.jsonl` (one line: `{"source": "...", "value": ...}`). TS and Python sides consume the same file — single source of truth, no per-runtime drift.
- `make emit-fixtures` target.

The cross-runtime consumer side (Python and TS test harnesses reading the fixtures) is tracked in those repos, not here. This row stops at the seam.

**Done when:** running `make test-differential` against `arb_expr(depth=4)` × 256 cases produces zero divergences against `formspec-py`; fixture file is reproducible (deterministic seed) and committed.

### C4. Resource-budget enforcement (`EvalBudget` seam) `[open]`

`src/parser.rs:36-49` caps parser recursion at 32 frames. `src/evaluator/core.rs` has a recursion cap (commit `76673bd`). There is no step count, allocation ceiling, or wallclock budget. `repeat("a", 10_000_000)`, deeply chained `map` over a huge array, or pathological regex (verify the `regex` crate's linear-time guarantee actually holds for every regex-bearing builtin) will allocate or spin without bound.

**Why now:** once the evaluator ships in untrusted contexts (case-portal, public SaaS, wos-server adapters), retrofitting a budget parameter through `evaluate*` signatures forces every downstream caller to migrate. Cheap now, expensive after the consumer set grows.

**Architectural seam:**

```rust
pub struct EvalBudget {
    pub max_steps: u64,
    pub max_alloc_bytes: u64,
    pub deadline: Option<Instant>,
}
```

Threaded through new `evaluate_with_budget`/`evaluate_with_budget_and_extensions` entry points; existing `evaluate` and `evaluate_with_extensions` (`src/evaluator/core.rs:214`, `:232`) delegate with `EvalBudget::unlimited()` so current callers are untouched. Exceeding the budget returns `EvalError::BudgetExceeded { kind }` — typed, never a panic, never a partial value.

**Test surface (`tests/budget_tests.rs`):**

- Pathological inputs (huge `repeat`, deep `map` chains, every regex-bearing builtin against a known catastrophic-backtracking pattern) terminate with `BudgetExceeded { kind: Steps | Alloc | Deadline }`.
- Chaos proptest (`tests/fel_chaos_proptest.rs`) is updated to evaluate every random AST under a small `EvalBudget`; the property is "either succeeds within budget or returns `BudgetExceeded` — never panics, never OOMs."

**Done when:** seam exists, default behavior unchanged for existing callers, budget tests green, chaos proptest updated.

---

## MEDIUM

### C5. Decimal property coverage extension `[open]`

Commit `76673bd` added checked Decimal arithmetic. Coverage today is implicit — the chaos proptest happens not to find overflow paths because its distribution is byte-level.

**Deliverable:** `tests/decimal_properties.rs` consuming C1's `arb_decimal`:

- For all `+ − × ÷` over `arb_decimal × arb_decimal`: result is `Ok(_)` or a typed `Overflow` / `DivByZero`. No panic, no NaN-equivalent.
- Coercion: `i64 → Decimal → i64` round-trips when in range; returns typed `OutOfRange` when not.
- JSON serialization (post-completed-item-9 in `src/convert.rs:125-128`): `Decimal → JSON string → Decimal` is bit-exact.
- Money: `money(a, "USD") + money(b, "USD")` is currency-stable; `money(a, "USD") + money(b, "EUR")` returns the typed mixed-currency error.

### C6. Fuzz-to-regression pipeline `[open]`

`fuzz/fuzz_targets/fel_pipeline.rs` and `fuzz/fuzz_targets/fel_structured.rs` exist. Nothing converts a libfuzzer crash into a permanent regression test — discovered bugs leave the corpus when the corpus is rotated.

**Deliverable:**

- `make fuzz-extract` — runs `cargo fuzz cmin` over the corpus, then `cargo fuzz tmin` on artifacts, emits minimized inputs as new rows in `tests/evaluator_edge_cases.rs` via `scripts/fuzz_to_regression.py`. Each minimized crash becomes a named test (slugged from the input bytes' SHA-256 prefix).
- `make fuzz-coverage` — wires `cargo fuzz coverage` and renders HTML to `fuzz/coverage/`.

### C7. Conformance corpus as fuzz seed + dictionary `[open]`

`fuzz/fuzz_targets/fel_structured.rs:9-19` uses 9 hardcoded seeds. `tests/function_semantics_conformance.rs` already enumerates real expressions; mutation can land far closer to grammar.

**Deliverable:**

- Build script (or `make seed-fuzz`) that dumps every conformance-suite source into `fuzz/corpus/fel_structured/`.
- `fuzz/fel.dict` emitted from the lexer keyword set (`src/lexer.rs`) and the builtin catalog (`src/extensions/catalog.rs`). libFuzzer reads the dictionary for grammar-aware mutation.

### C8. Coverage-guided gap audit `[open]`

One-shot: `cargo llvm-cov --tests --html`. Likely uncovered: builtin error paths in `src/evaluator/builtins/aggregates.rs`, `:strings.rs`, `:logic_types.rs`; decimal overflow rails in `src/evaluator/core.rs`.

**Deliverable:** focused tests closing every uncovered branch worth closing (judgment call — not chasing 100%, chasing "every branch a real input could hit"). Audit notes land in `thoughts/2026-05-07-coverage-audit.md`.

Run **after** C1–C5 so the report reflects the post-property-coverage state.

---

## LOW

### C9. Concurrency smoke `[open]`

Verify `MapEnvironment` is `Send + Sync`. No tests exercise concurrent evaluation against a shared `Arc<Env>`. Cheap insurance against any future change that loosens those bounds without test signal.

**Deliverable:** `tests/concurrency_smoke.rs` — `N` threads × `M` random ASTs from C1 against a shared `Arc<MapEnvironment>`; assert per-thread results equal a serial baseline. Fails loudly if `Send`/`Sync` is ever broken downstream.

### C10. Snapshot error messages `[open]`

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
