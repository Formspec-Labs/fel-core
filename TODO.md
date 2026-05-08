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

---

# 2026-05-08 multi-lens review (R21–R41)

Seed: 2026-05-08 swarm of three Sonnet reviewers — semi-formal-code-review, data-intensive-systems, platform-strategist. Validated 2026-05-08 by code-scout (opus) read-only pass against actual sources; corrections folded in (R24, R31, R32, R36) and five new findings appended (R37–R41). Treat the C1–C10 + R1–R20 "all done" narrative as a hypothesis to falsify; reopen rows where coverage or seam quality lags the claim.

Severity legend: **HIGH** = correctness or cross-runtime integrity; **MEDIUM** = coverage / surface area; **LOW** = doc / nit.

---

## HIGH — correctness (reopens C4)

### R21. Duplicate `budget exceeded (alloc)` diagnostics on breach `[done]`

`src/evaluator/core.rs:396-400` (`track_alloc`) emits the diag when `alloc_limit_breached()` fires; the **next** `eval()` then hits `check_budget()` at `:474-476`, which re-checks alloc and emits again. Repeats every recursion. `tests/budget_tests.rs` asserts `any(d.message.contains("budget exceeded"))` — never count, so invisible.

**Fix:** add `budget_breached: bool` flag on `Evaluator`; first emission sets, subsequent budget diags suppressed. Add a budget-test that asserts diagnostic **count = 1** for breach scenarios.

### R22. `BinaryOp::Concat` bypasses alloc budget entirely `[done]`

`src/evaluator/core.rs:1146-1157` produces `format!("{a}{b}")` without `track_alloc`. Every other heap-producing node (string literal, array, object, let) tracks. A deep Concat tree builds unbounded strings under `EvalBudget { max_alloc_bytes: small }`. C4's alloc ceiling is not actually enforced for the most common heap-grower in real expressions.

**Fix:** `track_alloc((a.len() + b.len()) as u64)` before format!; early-return on breach. Test: deeply nested `~~` chain with small alloc budget asserts `Null + budget diag`.

---

## HIGH — cross-runtime integrity (reopens C3)

### R23. TS/WASM differential oracle missing `[done]`

`tests/differential_oracle.rs` covers Rust↔Python only, 64 `arb_expr(depth=3)` cases, `#[ignore]`'d behind `make test-differential`. **No TS/WASM oracle exists.** Every WOS governance gate and case-portal evaluation that runs TS-side has no systematic parity guarantee with the Rust authority. Highest blast radius of any open finding — silent semantic drift in rights-impacting code paths.

**Architectural seam:** mirror the Python harness pattern with a node-side runner that imports `formspec-wasm` (or the TS engine's evaluator entry), reads expression+env JSON from stdin, emits result JSON. Rust harness shells out the same way.

**Done when:** parity test green at ≥256 cases over `arb_expr(depth=4)` against TS evaluator; `make test-differential` runs both Python and TS oracles; CI runs at least one of the two on every PR.

### R24. `power()` fractional/negative exponent falls to f64 — not in oracle corpus `[done]`

`src/evaluator/builtins/numeric.rs:64-80`: integer exponent uses a hand-written `checked_mul` loop over `exp.to_u64()` (28-29 digits, Decimal-clean); fractional or negative exponent falls to `base_f.powf(exp_f)` at `:78-80` (15-17 digits, f64). This is the single most likely Rust↔Python↔TS divergence point — a `power(1.0000001, -1)` could differ in the 16th decimal and feed a threshold comparison upstream. `arb_expr` is unlikely to generate this pattern in 64 random cases.

**Fix:** seed the oracle corpus explicitly with fractional/negative power cases (`tests/differential_oracle.rs` should include hand-picked fixture set in addition to proptest). Decide separately — lean toward documenting f64 fallback as non-conformant rather than introducing a Taylor / log-exp Decimal path that opens its own divergence axis vs. Python/TS.

---

## MEDIUM — coverage debt (reopens C1, C3, C6)

### R25. Phantom proptests in `tests/fel_proptest.rs` `[done]`

`:34` (`null_propagates_through_binary_numeric`), `:43` (`equality_no_null_propagation`), `:65` (`json_object_order_roundtrip`) all use `_ in any::<u8>()` and assert constant string-literal expressions. 128 iterations of one case each. Property infrastructure without property coverage.

**Fix:** either replace with meaningful generators (vary the operands via `arb_value`) or demote to plain `#[test]`. Lean toward the former — these properties have real generative space.

### R26. `parse_print_identity` skips most of `arb_expr` `[done]`

`tests/ast_proptest.rs:22-40`: five escape clauses (`.`, `let`, `if`, `then`, `[`) return `Ok(())` unconditionally. Field refs, let-bindings, conditionals, membership ops — all of `arb_expr`'s most interesting outputs — are skipped. C1's "parse(print(ast)) == ast over full AST" claim is overstated; coverage is literal/call subset only.

**Fix:** address the printer's asymmetric cases until the escape clauses can be removed. Each removal is one TDD cycle: pick the simplest skipped case (probably `let`), reproduce the round-trip failure, fix the printer, drop the escape.

### R27. Fuzz targets call unlimited `evaluate()` `[done]`

`fuzz/fuzz_targets/fel_pipeline.rs:17`, `fuzz/fuzz_targets/fel_structured.rs:42`. Budget code paths never fuzzed — neither R21 (duplicate diags) nor R22 (Concat alloc gap) discoverable by current fuzz.

**Fix:** third fuzz target `fel_budget` calls `evaluate_with_budget(&expr, &env, EvalBudget { max_steps: 500, max_alloc_bytes: 64*1024, deadline: None })`; asserts result is `Value::Null + budget diag` or valid `Value`, never panics.

### R28. Differential oracle swallows Python failures `[done]`

`tests/differential_oracle.rs:31-36` — `python_val()` returns `None` on any non-zero exit or non-JSON output. Python panics or unexpected non-parse failures pass silently. Oracle only fires when Python *succeeds*; error-path divergences invisible.

**Fix:** distinguish (a) Python parse error on input that Rust also rejects (fine, skip), (b) Python panic / unexpected error on input Rust evaluated (fail). Capture stderr; pattern-match against expected parse-error prose.

### R29. `arb_value` generates single-element arrays only `[done]`

`src/testing/strategies.rs:29-30` — `prop_map(|v| Value::Array(vec![v]))`. Aggregate functions (`sum`, `every`, `some`, `countWhere`, `avg`) on multi-element arrays unexercised by any proptest.

**Fix:** `prop::collection::vec(arb_value(depth-1), 0..4).prop_map(Value::Array)`.

### R30. Fuzz crash artifacts not replayed in CI `[done]`

`fuzz/artifacts/fel_pipeline/` contains committed crash inputs. `.github/workflows/doc.yml` is the only workflow; no harness replays the artifacts. A regression on those inputs is silent.

**Fix:** CI step runs `cargo +nightly fuzz run fel_pipeline -- -runs=0 fuzz/artifacts/fel_pipeline/*` (or equivalent replay-only invocation).

---

## MEDIUM — design / API surface

### R31. `EvalBudget` API hygiene — deadline vs. step regimes not named `[done]`

`src/evaluator/budget.rs:63-66` — `Instant::now()` syscall is gated behind `if let Some(deadline)`, so batch users with `deadline: None` pay zero per-step cost (original perf claim was overstated). The remaining substance is API hygiene: callers don't have a named entry for "I want this for batch" vs "I want this for interactive UI"; both regimes coexist in the same struct with no idiomatic constructor.

**Fix:** add `EvalBudget::for_batch(steps, alloc)` and `EvalBudget::for_interactive(deadline)` constructors. Doc-comment on `deadline` field clarifying it's for interactive/UI use (clock-bound) while `max_steps` is for throughput-bound batch / projection consumers.

## HIGH — correctness, take two (promoted from doc gap)

### R32. `ExtensionRegistry` results bypass alloc budget categorically `[done]`

**Validated 2026-05-08 — stronger than originally filed.** `src/evaluator/core.rs:1469-1479`: `registry.call(name, &evaluated_args)` returns a `Value` that is **never** run through `track_alloc`. This is not a doc gap — it is a categorical correctness gap. A 1-step extension call returning a 1GB string completely defeats `EvalBudget::max_alloc_bytes`. R21 / R22's resource-exhaustion ceiling does not bind for any consumer using extensions.

**Fix:** post-charge `track_alloc(value_size_estimate(&result))` immediately after `registry.call` returns; on breach, replace with `Value::Null` + `budget exceeded (extension result)` diag. Define `value_size_estimate` once (it has uses beyond extensions — see R38). Add test: registered extension returning a 256 KiB string under a 64 KiB alloc budget produces `Null + diag`. Spec the contract in `EvalBudget` rustdoc so future extension authors see the obligation.

---

## MEDIUM — platform positioning (cross-stack — escalate to TODO-STACK.md if owner agrees)

These four are positioning rows, not implementation rows. Filed here for traceability; ownership likely belongs at the stack root since they affect external posture for every consumer.

### R33. No semver / stability commitment, not on crates.io `[done]`

`README.md` documents the API but says nothing about breaking-change posture. The 2026-05-06 wave landed 30+ behavioral changes including `Value::Object` shape change, `length(null)` semantics, removed `Decimal` re-export — correct changes, no version signal. Path-coupled via `path = "../fel-core"` only; the Apache-2.0 + open-core procurement story doesn't deliver until crates.io publication exists.

**Fix:** decide semver posture (likely 0.x with declared breaking-change channel until 1.0). Publish to crates.io. Section in README pinning what counts as a breaking change (AST shape, builtin signature, error wire shape, public re-exports).

### R34. No external FEL semantics spec doc `[done]`

`BUILTIN_FUNCTIONS` schema covers the catalog but not evaluation semantics — null propagation, type coercion, operator precedence, budget model, error wire shape. An independent implementor cannot build a conformant FEL engine from public materials. VISION.md §Q4 commits "every MUST gets a passing fixture at 1.0"; that promise has no document yet.

**Fix:** `fel-core/docs/SPEC.md` covering grammar (link to PEG / parser source), evaluation rules (null propagation table, coercion table), builtin catalog (link to generated schema), `EvalBudget` contract, `Diagnostic` wire shape. Owner-facing question: does this live in fel-core, in formspec-site, or as a top-level FEL spec at the stack root?

### R35. External conformance corpus `[done]`

C3's differential oracle uses internal test data. STACK.md describes per-project conformance ownership; no externally downloadable corpus exists. Convert "internal test suite" → "third parties verify conformance" (the actual procurement-legible artifact).

**Fix:** extract corpus to JSON triples (expression / environment / expected output / expected diagnostic kinds) under `fel-core/conformance/`; one fixture file per builtin and per evaluation-semantic rule; CI generates corpus from the live test data so it cannot drift.

### R36. Diagnostic taxonomy partial; calendar internals leaked `[done]`

Two surface-area items rolled together since they're both README/lib.rs hygiene:
- README.md:37-39 already documents diagnostic `kind` with `undefinedFunction` and `typeMismatch` examples — original "lives in changelog only" framing was wrong. Substantive gap: only 2 kinds enumerated, no closed-taxonomy commitment, no link to the `DiagnosticKind` enum source. Promote to a proper README §Diagnostics section enumerating the full closed set, with a stability commitment ("kinds are append-only; existing kinds are stable through 1.0").
- `src/lib.rs:75-78` re-exports `civil_from_days`, `days_from_civil`, `days_in_month`, `parse_date_literal`, `parse_datetime_literal` as public surface. No documented external consumer; creates stability obligation. Move to `pub(crate)` — feature-gating is over-engineering for items with no external use case.

---

## New observations from opus validation pass (R37–R41)

### R37. `make_string` / value-constructor seam — prevent future Concat-style omissions  `[done]`

While verifying R22, opus noted that alloc-tracking discipline is enforced node-by-node rather than through a constructor helper. Each `Value::String(...)` / `Value::Array(...)` construction site manually orchestrates `track_alloc` + `alloc_limit_breached` + `Value::Null`-or-real-value. The smell is feature envy: a `fn make_string(&mut self, s: String) -> Value` (and siblings for Array, Object) collapses the orchestration into one place, closes the door on R22-shaped omissions structurally, and pairs naturally with R32's `value_size_estimate`.

**Fix:** introduce `Evaluator::make_{string,array,object}(&mut self, …) -> Value` helpers; migrate all heap-producing eval sites (literals, Concat, builtin returns, extension results, JSON parsing) to use them; remove the manual `track_alloc` calls at construction sites. Property: every public path that produces a heap value is unreachable without the budget check.

### R38. `EvalBudget::tiny()` misnamed  `[done]`

`src/evaluator/budget.rs:46-52` — `EvalBudget::tiny()` doc-comment says "smallest budget guaranteed to allow at least one evaluation step" but constants are `max_steps: 1, max_alloc_bytes: 1024`. 1024 bytes is not "tiny" relative to step-1 work; the name overcommits.

**Fix:** rename to `EvalBudget::min_viable()`, OR split into `for_one_step()` (steps: 1, alloc: 1024) and a true `tiny()` (steps: 1, alloc: 16). Pick whichever matches actual call-site patterns in `tests/budget_tests.rs`.

### R39. `MAX_STRATEGY_DEPTH` constant is dead  `[done]`

`src/testing/strategies.rs:20-21` — `#[allow(dead_code)] const MAX_STRATEGY_DEPTH: u32 = 4;` never referenced. Either wire it as the default depth in `arb_expr` / `arb_value` (consolidating the magic-number `depth` param defaults) or delete. The `#[allow(dead_code)]` annotation is the smell — it says "I know this is dead but I'm keeping it" without saying why.

**Fix:** wire as the default depth — every test passes `depth: 4` already, the constant just makes the convention explicit.

### R40. `evaluate*` API surface is parameter-object smell at full volume  `[done]`

`src/lib.rs:51-56` exposes 8 entry points: `evaluate`, `evaluate_with_trace`, `evaluate_with_extensions`, `evaluate_with_budget`, `evaluate_with_trace_and_extensions`, `evaluate_with_trace_and_budget`, `evaluate_with_extensions_and_budget`, `evaluate_with_trace_and_extensions_and_budget`. The combinatorial naming is a load-bearing signal that the parameter set is wrong.

**Fix:** introduce `EvaluatorOptions { trace: Option<&mut Trace>, extensions: Option<&ExtensionRegistry>, budget: EvalBudget }` with `Default::default()` returning unlimited / no-trace / no-extensions; collapse to `evaluate(&Expr, &dyn Environment) -> EvalResult` and `evaluate_with(&Expr, &dyn Environment, EvaluatorOptions) -> EvalResult`. Pairs with R33 (semver decision) — this is a breaking change worth landing pre-1.0.

### R41. `Concat` is the worst untracked heap-grower but other format!-bearing paths exist  `[done]`

While confirming R22, opus noted format!-bearing diagnostic-error paths exist throughout (e.g. error-message construction in builtins). Those are bounded by message-template length, not adversarial input, so they are not exploitable like Concat. Flagging for posterity — once R37 (`make_string` helper) lands, the value-vs-diagnostic distinction makes this a non-issue. **Lean: defer / close-as-no-op once R37 lands.**
