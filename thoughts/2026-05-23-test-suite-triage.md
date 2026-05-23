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

Five clusters of identical-shape repetition. **Hard contract: consolidate only same-shape, same-contract tests. Diagnostic-message-content tests stay standalone; span-assertion tests stay standalone; fuzz/regression-corpus tests stay standalone.**

| Cluster | Files | Tests → Table | Notes |
|---|---|---:|---|
| **A. Money — operator arithmetic** | `evaluator_edge_cases.rs:111-228` | 12 → 1 | `money op money/scalar`, currency-mismatch null-prop. KEEP separate: 6 diagnostic-content tests in `evaluator_tests.rs` (`test_money_*_diagnostic`, `test_sum_rejects_*`) — they pin message text, not just `Value::Null`. |
| **A′. Money — builtin functions** | `evaluator_tests.rs:577-636` | 4 → 1 | `money()`, `moneyAdd()`, `moneyCurrency_mismatch`. Constructor/accessor + builtin dispatch contract. Distinct from A (operator overload). |
| **B. Date arithmetic** | `evaluator_edge_cases.rs:233-365` + `evaluator_tests.rs:491-497` | 11 → 1 | `test_date_add` (evaluator_tests.rs:497) is a literal duplicate of `date_add_month_day_clamping` (edge_cases:331) — delete the duplicate, consolidate the rest. |
| **C. Parser rejection** | `parser_rejection_tests.rs:1-285` only | 42 → 1 | Per-row tuple **must** carry `(input, spec_section, rejection_intent)` — structured column, not freeform comment (review H1). |
| **C′. Parse-error span correctness** | `parser_rejection_tests.rs:286-321` | 3 → keep as-is | Span-byte-range assertions — different shape from rejection. EXCLUDE from C table. |
| **D. Regex — homogeneous shapes** | `regex_tests.rs` + `evaluator_tests.rs:1069-1145` (cross-file merge) | ~28 → 2–3 | Anchors / quantifiers / character-classes consolidate. EXCLUDE: locale-sensitive, back-reference rejection, diagnostic-shape tests — they stay standalone. Pull regex tests out of `evaluator_tests.rs` so the contract surface lives in one file. |
| **E. Object/array equality** | `evaluator_edge_cases.rs:367-457` | 9 → 1 | 5 object + 4 array equality cases. Uniform `assert_eq!(eval, Boolean(_))` shape. |

### Cluster contract requirements (per review)

- **Tables must surface intent per row, not per table.** When a cluster mixes contracts (operator vs builtin in A, anchors vs quantifiers in D), add a `kind`/`contract` column to the tuple, or split into two tables.
- **Diagnostic-asserting tests are out of scope for Phase 1 consolidation.** They pin specific message text (`"use moneySum()"`, `"moneyAmount("`, etc.); merging weakens those assertions.
- **Spec citations are structured per-row.** Cluster C row tuple: `(input, spec_ref, intent)`. Row comments drift; columns get reviewed.
- **Fuzz/regression-corpus tests are out of scope.** `fuzz_regression_corpus` (FEL-SMELL-C-001) at `evaluator_edge_cases.rs:672-708`, `decimal_multiplication_overflow_is_null_not_panic`, and `evaluation_depth_limit_returns_null_with_diagnostic` (the LibFuzzer guards at `evaluator_edge_cases.rs:22-56`) stay as bespoke tests — they pin specific historical panics with bespoke setup (manual deep-AST construction, `std::mem::forget`).

After Phase 1, **delete `evaluator_edge_cases.rs` only if empty** after migration. Expected residue: the LibFuzzer guards + `fuzz_regression_corpus`. If those remain, narrow the file's scope to "fuzzer regression guards + corpus" via the module doc-comment.

**`*.proptest-regressions` seed files (3 of them) are untouched** by consolidation — they're persistent proptest failure seeds, not test bodies.

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
- [x] Pre-Phase-1 architecture review dispatched (`semi-formal-architecture-review`) — 1 HIGH + 4 MED + 3 NIT, all remediated in plan doc (see Deviations §1)
- [x] Cluster **A** (money operator arithmetic) — 12 → 1 (sha 98e77bb, −49 LOC, suite green)
- [x] Cluster **A′** (money builtin functions) — 3 → 1 (sha 86ae369; diagnostic-message test stays standalone)
- [x] Cluster **E** (object/array equality) — 9 → 1 (sha 3939df7, −31 LOC)
- [x] Cluster **B** (date arithmetic) — 11 → 1 + deleted `test_date_add` duplicate (sha cae03c9, −100 LOC across two files)
- [x] Code review checkpoint (`semi-formal-code-review`, every 3–5 commits) — 3 reviewer agents (formspec-scout, test-engineer, formspec-test) returned with HIGH H1 + MED + LOW; all remediated (shas 1cbac25, 043963e)
- [x] Cluster **C** (parser rejection + valid-parse contrast) — 42 → 7 (sha 363cab9). Then redesigned per user feedback to typed `Cite` enum + named consts (sha 42f35b3).
- [x] Cluster **D** (regex) — 47 tests across 2 files → 3 tests in `regex_tests.rs` with failure-collection (sha f6ecb3c)
- [x] Redistribute `evaluator_edge_cases.rs` into topic-canonical sections (sha 984c6b1) — file deleted; LibFuzzer guards → `evaluator_regression_guards.rs`; fuzz corpus → `fuzz_regression_corpus.rs`; everything else merged into `evaluator_tests.rs` at topic sections.
- [x] `make test-differential` green (conformance corpus untouched; differential_oracle gated behind `#[ignore]` per existing setup)
- [x] Post-Phase-1 architecture review (`semi-formal-architecture-review`) — ACCEPT verdict, 0 BLOCKER/HIGH/MEDIUM, 2 NITs both justified-rejected
- [x] Final code review pass — 1 MEDIUM (M1: helper deduplication) + 1 NIT (N1: §3.4.1 typo); both remediated (sha c7d3f01)
- [x] All review findings remediated (BLOCKER/HIGH → fix; warnings → fix or justified inline; nits → cleaned) — four rounds: pre-Phase-1 (1H+4M+3N), mid-phase across 3 reviewers (1H+5M+3N), user-flagged hackiness, post-Phase-1 final (1M+1N)
- [x] `cargo test` green (131 evaluator-domain tests + ~700 elsewhere); `cargo fmt --check` clean; `cargo clippy --tests --all-features` clean

**Phase 2 — Mutation gate** (per-file LOC counts omitted per stack decay-class rules):
- [ ] Install `cargo-mutants`
- [ ] Run on `src/parser.rs` — record surviving mutants
- [ ] Run on `src/evaluator/core.rs`
- [ ] Run on `src/dependencies.rs`
- [ ] Run on `src/convert.rs`
- [ ] Run on `src/error.rs`
- [ ] Run on `src/evaluator/builtins/{money,dates}.rs`
- [ ] Wire to CI as nightly (not per-PR) job
- [ ] First 5 unkilled mutants per file → file follow-up issues (one per gap)

**Phase 3 — Leverage policy in CI**:
- [ ] Proptest for `extract_dependencies` (P0 seam, currently uncovered)
- [ ] Proptest for `prepare` / `prepare_for_host` (P0 seam, currently uncovered)
- [ ] CI gate: each public `lib.rs` re-export requires ≥1 proptest before example tests count
- [ ] Post-Phase-3 architecture review

**Closeout**:
- [x] Parent-repo (`formspec-stack/`) submodule pointer bump prepared — HELD for owner-approved push (per directive). fel-core `main` at sha c7d3f01.
- [x] Final verification: `cargo test` green (548 #[test] runner counts across 30 binaries, including the failure-collection tables that themselves cover 60+ matches rows, 33 parser rejections, etc.); `cargo fmt --check` clean; `cargo clippy --tests --all-features` clean.

## Deviations

This section is **append-only**. Any divergence from the plan above (skipped step, added step, ad-hoc steering, scope change) gets a short numbered entry here so audit can reconstruct the actual path.

1. **Pre-Phase-1 architecture review (semi-formal-architecture-review against sha 3d45c95) returned 1 HIGH + 4 MEDIUM + 3 NIT.** All remediated in the plan doc *before* any code change:
   - **H1** — Cluster C requires structured per-row `(input, spec_ref, intent)` tuples, not freeform comments. Encoded in §"Cluster contract requirements".
   - **M1** — Cluster A split: money-operator-arithmetic (A) and money-builtin-functions (A′) are distinct contracts; diagnostic-message-content tests stay standalone.
   - **M2** — `parser_rejection_tests.rs:286-321` span-assertion tests excluded from Cluster C, broken out as C′ (kept as-is).
   - **M3** — `fuzz_regression_corpus` + LibFuzzer guards explicitly out of scope; `make test-differential` added to closeout.
   - **M4** — Cluster D includes cross-file pull from `evaluator_tests.rs:1069-1145`.
   - **N1** — "Delete edge_cases entirely" softened to "delete if empty after migration; else narrow."
   - **N2** — Phase 2 LOC counts removed (decay-class).
   - **N3** — `*.proptest-regressions` seeds noted as untouched.

2. **Mid-phase code review (3 reviewer agents in parallel)** dispatched after clusters A/A′/E/B landed:
   - **formspec-scout (semi-formal-code-review)**: CLEAN verdict. Found code-review M1 (`MoneyCase::Number(i64)` future-proofing) and N1 (`DateOp::Date` shadow). Remediated in 043963e.
   - **test-engineer (drop-in review)**: MEDIUM #1 (date match-guard vs assert_eq! inconsistency), MEDIUM #2 (first-fail-hides-rest on 33-row parser table), LOW naming divergence, LOW Null-overload, LOW asymmetry, NIT pointer comments. Failure-collection pattern propagated to all big tables (sha f6ecb3c for regex; sha 42f35b3 for parser).
   - **formspec-test (drop-in review)**: **HIGH H1 — Cluster C spec citations were demonstrably wrong** against actual `specs/fel/fel-grammar.md`. `§7 L385-386` was inside §6.3.1 host bindings (not pipe); `§7 L376-377/L374-375` was §6.3 host-binding prose (not rejection rule). Pre-existing rot survived the comment→column promotion because first review didn't re-check content. Remediated to actual rule positions (§7 L501-503 for rejection, §7 L510-512 for pipe) in sha 1cbac25. Plus NIT N2 (`spec.md` → `docs/SPEC.md`).
   - **Cross-cutting lesson** (formspec-test): "The H1 column-vs-comment fix is necessary but not sufficient — reviewer attention must re-check citation content, not just the column structure." Logged.

3. **User-flagged hackiness in `parser_rejection_tests.rs`** (mid-execution): the `type SpecRef = &'static str` and `type Intent = &'static str` aliases provided no type safety; the sentinel string `"non-spec (correctness)"` overloaded the citation column; rustfmt blew rows to 4-5 lines each. Redesigned (sha 42f35b3): typed `enum Cite { Grammar { section, lines }, Policy }`; named `const G_*` items deduplicate citations across rows (one edit fixes N rows pinning the same rule); plus failure-collection across 33 rows. Substantially cleaner; future spec-line drift hits one line, not eight.

4. **User-flagged topology in `evaluator_edge_cases.rs`** (mid-execution): "shouldn't 'edge cases' just be part of the relevant tests? like money edge cases → with money tests?". Correct critique — the file was an audit-finding bolt-on, never reconciled with topic-canonical sections. Redistributed (sha 984c6b1): money tests → §Money in evaluator_tests.rs; date → §Date; equality → §Comparison; etc. LibFuzzer regression guards → `tests/evaluator_regression_guards.rs`; fuzz corpus → `tests/fuzz_regression_corpus.rs`. `evaluator_edge_cases.rs` DELETED. Five duplicates dropped along the way (length_of_null, length_of_array folded into test_string_functions, number_cast_invalid_string already exists, undefined_function_diagnostic subset of test_undefined_function, empty_edge_cases folded into test_empty_present).

5. **Post-Phase-1 reviews (architecture + code, parallel)** — ACCEPT verdict overall. Architecture review: 0 BLOCKER/HIGH/MEDIUM, 2 NITs both justified-rejected (separate regression-guards and fuzz-corpus files is intentional; LOC overage vs original target buys reviewer-checkable structure). Final code review: 1 MEDIUM (`common::eval_result` added but not consumed by `evaluator_regression_guards.rs`/`regex_tests.rs` which retained local helpers) + 1 NIT (pre-existing typo `S3.4.1` → `§3.4.1` at evaluator_tests.rs §Decimal precision section header). Both remediated in sha c7d3f01. Zero open findings.

## Phase 1 closure

**Status**: Phase 1 complete. fel-core `main` at sha c7d3f01.

**Net delta**:
- Test functions: 497 → 381 (−116, −23%)
- Test LOC: 6527 → 6027 (−500, −7.7%)
- Files: `tests/evaluator_edge_cases.rs` deleted; `tests/evaluator_regression_guards.rs` + `tests/fuzz_regression_corpus.rs` added; 5 existing files restructured.
- Conformance / proptest / differential-oracle: byte-identical (`git diff 3d45c95..HEAD --` empty for those paths).
- Cluster-by-cluster receipts:
  - A (money operator): 12 → 1 table (sha 98e77bb)
  - A′ (money builtins): 3 → 1 table (sha 86ae369)
  - B (date arithmetic): 11 → 1 table + duplicate deleted (sha cae03c9)
  - C (parser rejection): 42 → 7 then redesigned with typed Cite enum (shas 363cab9, 42f35b3)
  - D (regex): 47 across 2 files → 3 in regex_tests.rs (sha f6ecb3c)
  - E (equality): 9 → 1 table (sha 3939df7)
  - Remediation passes: 1cbac25 (HIGH H1 citation fix), 043963e (A/B/E pattern harmonization), 984c6b1 (edge_cases redistribution), c7d3f01 (final M1+N1)

**Gates green**: `cargo test`, `cargo fmt --check`, `cargo clippy --tests --all-features` all clean.

**Phase 2 (mutation gate) and Phase 3 (proptest gaps + CI policy) remain deferred**, both with explicit follow-up checklists above. They are infrastructure work (CI wiring, cargo-mutants install) deliberately scoped outside Phase 1's "consolidate without behavior loss" mandate.
