# Test suite triage — fel-core

**Date:** 2026-05-23
**Method:** Gut-Instinct strategy for AI-written test suites — seams-first, delete-then-consolidate-then-property-test.

## TL;DR

- **6527 LOC across 27 test files; 497 test fns.**
- **No dead tests.** All `#[ignore]` are gated conformance (`make test-differential`); none are forgotten.
- **Strong leverage tests already exist** — 7 property-test files + 4 conformance corpora. fel-core is *not* a typical AI-written suite; it already practices property-tests-per-seam.
- **Real rot is duplication + shape repetition**, not shallowness in isolation. Two "audit-finding response" files bolted on tests instead of consolidating: `evaluator_edge_cases.rs` (708 LOC) and `parser_rejection_tests.rs` (321 LOC).
- **Phase 1 alone removes ~700 LOC (-11%), collapses 90+ test fns to ~10 table-driven tables**, no behavioral coverage loss.

## Strategy applied

1. **Seams first, tests second.** Read `src/lib.rs` re-exports — classify every test by which public contract it pins.
2. **Delete dead tests.** No-op here.
3. **Coverage delta via cheap signals.** Static heuristics: assertion count, file size, shape repetition.
4. **Mutation testing on the 3–5 P0 seams.** Identified; deferred (cargo-mutants not installed).
5. **Cluster by assertion shape.** Five clear clusters identified.
6. **Kill brittle mocks last.** N/A — fel-core has no IO seams.

## Seam → test map (top public contracts)

| Seam (`lib.rs` re-export) | Production LOC | Tests pinning it |
|---|---:|---|
| `parse` / `tokenize` | parser.rs 1295 + lexer.rs | lexer_tests, parser_rejection_tests, parser_parse_proptest, ast_proptest, fel_chaos_proptest |
| `evaluate` / `Evaluator` | evaluator/core.rs 1831 | evaluator_tests, evaluator_edge_cases, semantic_invariants, decimal_properties, stress_tests, concurrency_smoke |
| `extract_dependencies` | dependencies.rs 524 | environment_integration_tests (partial) — **GAP: no proptest** |
| `json_to_fel` / `fel_to_json` | convert.rs 588 | fel_proptest, decimal_properties (partial) |
| `DiagnosticKind` | error.rs 599 | snapshot_tests, builtin_catalog_consistency, evaluator_tests (partial) |
| `Trace` / `evaluate_with` | trace.rs | trace_tests |
| `EvalBudget` | evaluator/budget.rs | budget_tests, stress_tests |
| `prepare` / `prepare_for_host` | prepare_host.rs | host_bindings — **GAP: no proptest** |
| Cross-runtime parity | n/a | differential_oracle, public_conformance_corpus, function_semantics_conformance, schema_round_trip |

## Cheap signals collected

| Signal | Finding |
|---|---|
| Shallow-test ratio (≤1 assertion per fn) | edge_cases 71%, regex 97%, parser_rejection 93%, lexer 84%, locale 65% |
| Top assertion counts (integration-masquerading-as-unit) | lexer:public_tokenize 16, evaluator:builtin_type_arity 14, lexer:all_punctuation 11, evaluator:string_functions 10 |
| Mock count per test | 0 (no IO seams) |
| Import depth | Mostly `fel_core::*` glob; no internal-pin smells |
| Property-test coverage per public seam | Strong for lex/parse/eval; **missing for `extract_dependencies` and `prepare_host`** |

## Phase 1 — Consolidation clusters (high leverage, low risk)

Five clusters of identical-shape repetition. Estimated ~700 LOC removed, 90+ test fns → ~10 table-driven tests, behavior preserved.

| Cluster | Files | Tests → Table | LOC Δ |
|---|---|---:|---:|
| **A. Money arithmetic** | `evaluator_edge_cases.rs:111-232` + `evaluator_tests.rs` (scattered, 14) | 26 → 1 | −190 |
| **B. Date arithmetic** | `evaluator_edge_cases.rs:233-365` + `evaluator_tests.rs:491-497` | 11 → 1 | −110 |
| **C. Parser rejection** | `parser_rejection_tests.rs` (entirely) — preserve spec citations as row comments | 42 → 1 | −240 |
| **D. Regex** | `regex_tests.rs` (97% single-assert) — group by feature | 36 → 2–3 | −180 |
| **E. Object/array equality** | `evaluator_edge_cases.rs:367-457` | 8 → 1 | −65 |

After Phase 1, **delete `evaluator_edge_cases.rs` entirely** if its consolidated tables migrate into `evaluator_tests.rs` next to related material — the "edge cases" framing is its own smell (an audit response that never got reconciled).

No new dependency needed. Plain Rust array iteration in a single `#[test]` fn is sufficient and idiomatic.

### Consolidation example — Cluster A (money arithmetic)

**Before** (26 separate test fns across two files, each ~10 LOC):

```rust
#[test]
fn money_subtraction() {
    let result = eval("money(100, 'USD') - money(30, 'USD')");
    match result {
        Value::Money(m) => {
            assert_eq!(m.amount, Decimal::from(70));
            assert_eq!(m.currency.as_str(), "USD");
        }
        _ => panic!("expected money, got {result:?}"),
    }
}

#[test]
fn money_multiply_by_scalar() {
    let result = eval("money(25, 'EUR') * 4");
    match result {
        Value::Money(m) => {
            assert_eq!(m.amount, Decimal::from(100));
            assert_eq!(m.currency.as_str(), "EUR");
        }
        _ => panic!("expected money, got {result:?}"),
    }
}
// ... 24 more
```

**After** (one table-driven test, ~60 LOC for 26 cases):

```rust
#[test]
fn money_arithmetic_table() {
    enum Expected {
        Money(&'static str, &'static str), // (amount, currency)
        Number(i64),
        Null,
    }
    let cases: &[(&str, Expected)] = &[
        ("money(100, 'USD') - money(30, 'USD')",  Expected::Money("70", "USD")),
        ("money(25, 'EUR') * 4",                  Expected::Money("100", "EUR")),
        ("3 * money(10, 'GBP')",                  Expected::Money("30", "GBP")),
        ("money(100, 'USD') / 4",                 Expected::Money("25", "USD")),
        ("money(100, 'USD') / money(25, 'USD')",  Expected::Number(4)),
        ("money(100, 'USD') / money(25, 'EUR')",  Expected::Null), // currency mismatch
        ("money(100, 'USD') - money(30, 'EUR')",  Expected::Null),
        ("money(100, 'USD') / 0",                 Expected::Null),
        // ... rest
    ];
    for (input, expected) in cases {
        let actual = eval(input);
        match expected {
            Expected::Money(amt, cur) => match &actual {
                Value::Money(m) => {
                    assert_eq!(m.amount, Decimal::from_str(amt).unwrap(), "input={input}");
                    assert_eq!(m.currency.as_str(), *cur, "input={input}");
                }
                _ => panic!("input={input}: expected money, got {actual:?}"),
            },
            Expected::Number(n) => assert_eq!(actual, num(*n), "input={input}"),
            Expected::Null => assert_eq!(actual, Value::Null, "input={input}"),
        }
    }
}
```

Adding a case is one row, not one function. Drift between cases (different assertion shape) becomes impossible.

## Phase 2 — Mutation gate on critical seams

Install `cargo-mutants` and run nightly (not per-PR) on the P0 seams:

| File | LOC | Why P0 |
|---|---:|---|
| `src/parser.rs` | 1295 | Everything downstream is shadow |
| `src/evaluator/core.rs` | 1831 | Null propagation, decimal, MIPs |
| `src/dependencies.rs` | 524 | Engine reactivity graph |
| `src/convert.rs` | 588 | Wire-format with TS + Python |
| `src/error.rs` | 599 | Closed/append-only `DiagnosticKind` public API |

Plus `src/evaluator/builtins/{money,dates}.rs` for operational arithmetic.

Each surviving mutant ⇒ one shallow test. Stop at the first 5 unkilled mutants per file; that's the gap list.

```sh
cargo install cargo-mutants
cargo mutants --file src/parser.rs --timeout 60
```

## Phase 3 — Leverage policy in CI

Codify the Gut-Instinct rule: **every public contract in `lib.rs` requires at least one proptest before example-based tests count toward coverage.**

Today, `extract_dependencies` and `prepare_host` lack proptests despite being P0. Fix those gaps before piling on more example tests.

## What's NOT a problem here

- **Mocks.** fel-core is a pure library with no IO; no mocks to kill.
- **Brittle implementation pinning.** Property tests already assert at the algebraic-law layer (see `semantic_invariants.rs`).
- **Coverage of cross-runtime parity.** `differential_oracle` + 3 corpora already cover this.
- **`power_builtin.rs`** (96 LOC dedicated to one builtin) — initially a smell, but justified by FEL-SMELL-C-002 (exponent cap, perf-bounded). Keep.

## Net delta if executed

| Metric | Before | After Phase 1 | After Phases 1+2+3 |
|---|---:|---:|---:|
| Test LOC | 6527 | ~5800 (-11%) | ~5800 + mutation-driven gaps filled |
| Test functions | 497 | ~410 | ~410 + ~10 new proptests |
| Suite shape | "two big buckets + many small" | "table-driven per concept" | "property-anchored per seam" |

**Keep behavior. Delete structure. The suite shrinks while behavioral coverage strengthens** (table-driven enforces uniform assertions per case).

## Execution checklist

**Preconditions** (verify before starting):
- [x] `fel-core/` working tree clean (no unrelated edits)
- [x] `cargo test` green on baseline (all 29 test groups passing)
- [x] `cargo fmt --check` clean (verified post-doc-edit; doc-only changes don't affect)
- [x] On `main`

**Phase 1 — Consolidation** (one fel-core commit per cluster):
- [ ] Pre-Phase-1 architecture review dispatched (`semi-formal-architecture-review`)
- [ ] Cluster **A** (money arithmetic) — 26 → 1 table-driven test
- [ ] Cluster **E** (object/array equality) — 8 → 1 (validates template generalizes)
- [ ] Cluster **B** (date arithmetic) — 11 → 1
- [ ] Code review checkpoint (`semi-formal-code-review`, every 3–5 commits)
- [ ] Cluster **C** (parser rejection) — 42 → 1
- [ ] Cluster **D** (regex) — investigate grouping; 36 → 2–3
- [ ] Delete `evaluator_edge_cases.rs` if migration empties it
- [ ] Post-Phase-1 architecture review (`semi-formal-architecture-review`)
- [ ] Code review checkpoint
- [ ] All review findings remediated (BLOCKER/HIGH → fix; warnings → fix or justified inline; nits → cleaned)
- [ ] `cargo test` green; `cargo fmt`/`clippy` clean

**Phase 2 — Mutation gate**:
- [ ] Install `cargo-mutants`
- [ ] Run on `src/parser.rs` (1295) — record surviving mutants
- [ ] Run on `src/evaluator/core.rs` (1831)
- [ ] Run on `src/dependencies.rs` (524)
- [ ] Run on `src/convert.rs` (588)
- [ ] Run on `src/error.rs` (599)
- [ ] Run on `src/evaluator/builtins/{money,dates}.rs`
- [ ] Wire to CI as nightly (not per-PR) job
- [ ] First 5 unkilled mutants per file → file follow-up issues (one per gap)

**Phase 3 — Leverage policy in CI**:
- [ ] Proptest for `extract_dependencies` (P0 seam, currently uncovered)
- [ ] Proptest for `prepare` / `prepare_for_host` (P0 seam, currently uncovered)
- [ ] CI gate: each public `lib.rs` re-export requires ≥1 proptest before example tests count
- [ ] Post-Phase-3 architecture review

**Closeout**:
- [ ] Parent-repo (`formspec-stack/`) submodule pointer bump prepared for owner approval
- [ ] Final verification: `cargo test` + `cargo fmt --check` + `cargo clippy -- -D warnings`

## Deviations

This section is **append-only**. Any divergence from the plan above (skipped step, added step, ad-hoc steering, scope change) gets a short numbered entry here so audit can reconstruct the actual path. Empty at start of execution.
