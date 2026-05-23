# Mutation gate — survivor triage follow-ups

**Origin:** Phase 2 of [`2026-05-23-test-suite-triage.md`](./2026-05-23-test-suite-triage.md).
**Created:** with the initial mutation baseline at sha `a7faaf5` (Phase 2 infrastructure landing).

Per the plan's classification policy, every surviving mutant gets triaged into one of:

- **`kill`** — write a test (referencing the spec section that motivates the behavior)
- **`equivalent`** — annotate in `.cargo/mutants.toml` `skip_calls` (or the relevant skip mechanism) with a one-line justification
- **`accept`** — low-value mutation in non-load-bearing code (rare; default is `kill` or `equivalent`)

This document is the worklist. Each row is an open follow-up. Close by either landing a kill test (with the test's commit sha referenced) or moving the mutant to the equivalent-mutant skip list in `.cargo/mutants.toml`.

## Initial survivor list

Captured from the baseline run at sha `a7faaf5`. Big files (parser, lexer, evaluator/core, prepare_host) are still being baselined; this document will be appended as those rows arrive.

### `src/evaluator/budget.rs` — 4 missed / 8 viable (50% kill rate)

All four are `>` ↔ `>=` and `>` ↔ `==` mutations in `EvalBudget::check` at lines 77 and 80. These are **boundary mutations** — they survive when the test suite doesn't probe the exact `step_limit == steps_used` and `deadline == now` boundary cases.

| Mutant | Line | Triage |
|---|---|---|
| `> with ==` in check | 77 | **kill** — add a test that calls `check()` with `steps_used == step_limit`; expect `Err(StepLimit)` |
| `> with >=` in check | 77 | **kill** — covered by the same boundary test |
| `> with ==` in check | 80 | **kill** — add a test where `now == deadline`; expect `Err(Deadline)` |
| `> with >=` in check | 80 | **kill** — covered by the same boundary test |

### `src/dependencies.rs` — 10 missed / 23 viable (56.5% kill rate, below 80% floor)

Worst kill rate among baselined files. Survivors cluster around (a) `extend_field_path` match-arm deletions and (b) `dependencies_to_json_value*` return-value mutations. Investigate; likely several test gaps in `tests/environment_integration_tests.rs` and the proptest coverage that the plan's §"Phase 3 leverage policy" already flags as missing.

Open items:

- `match arm Expr::PostfixAccess{expr, path}` deletion at `:269` — postfix-access dependency extraction not exercised by current tests. **kill**: write a test that calls `extract_dependencies` on a `PostfixAccess` expression and asserts the path renders correctly.
- `dependencies_to_json_value -> serde_json::Value with Default::default()` at `:282` — JSON projection function returns Value(()) instead of the real graph. **kill**: assert JSON shape, not just round-trip equality.
- `dependencies_to_json_value_styled -> serde_json::Value with Default::default()` at `:290` — same shape, styled variant.
- Six more survivors — enumerate from `mutants.out/missed.txt` when re-running baseline.

This file is a strong candidate for the Phase 3 `extract_dependencies` proptest gap; landing that proptest will likely raise the kill rate substantially without per-mutant kill tests.

### `src/evaluator/builtins/money.rs` and `src/evaluator/builtins/dates.rs`

Pending — money+dates run still in flight at the time of this doc creation.

### `src/convert.rs` — 6 missed / 21 viable (71.4% kill rate, below 80% floor)

Six survivors. One observed: `delete match arm Value::String(s) in json_to_fel` at `:94` — string-branch deletion survives, meaning either the test suite doesn't pass JSON strings through `json_to_fel`, or it does but doesn't assert on the result strongly enough.

- **kill** — add a `fel_proptest.rs` row that round-trips JSON strings through `json_to_fel` and asserts on `Value::String` extraction.

### `src/error.rs` — 7 missed / 40 viable (82.5% kill rate ✓ above 75% floor)

Above floor. Survivors should be triaged but the file is in good shape overall.

- `replace == with != in has_error_diagnostics` at `:295` — the discriminator that distinguishes error-severity diagnostics from non-error doesn't catch this flip. **kill** or **equivalent**: investigate whether the test suite has a row with both Error and Warning diagnostics in the same `Vec<Diagnostic>`.
- Six more — enumerate from `mutants.out/missed.txt`.

### `src/extensions/registry.rs` — 3 missed / 14 viable (78.6% kill rate)

- `ExtensionRegistry::get -> Option<&ExtensionFunc> with None` at `:100` — registry's `get` always returns None survives → tests don't actually invoke the registered function via `.get()`.
- `ExtensionRegistry::contains -> bool with true` at `:105` — symmetric, `contains` returns true always.
- `<impl Display for ExtensionError>::fmt -> Result with Ok(Default::default())` at `:45` — Display impl not asserted on.

**kill** for the first two: add a registry test that registers a function, calls `.get(name)`, calls the returned fn, asserts on the result. **equivalent** likely for the third (Display formatting is usually exercised only by panic paths).

### `src/extensions/catalog.rs` — 1 missed / 2 viable (50% kill rate)

`builtin_function_catalog -> &'static[BuiltinFunctionCatalogEntry] with Vec::leak(Vec::new())` at `:44` — the catalog returns an empty slice, but the test suite (`builtin_catalog_consistency.rs`) passes anyway. This is concerning: it implies the consistency test doesn't actually iterate the catalog.

**kill** — must add: a test that asserts `builtin_function_catalog().len() > 0` and that at least one expected entry (e.g., `sum`, `if`) is present by name.

## Per-file kill-rate snapshot (sha `a7faaf5`)

| File | Killed | Missed | Total Viable | Kill Rate | Floor | Status |
|---|---:|---:|---:|---:|---:|---|
| `evaluator/budget.rs` | 4 | 4 | 8 | 50.0% | — | below typical |
| `dependencies.rs` | 13 | 10 | 23 | 56.5% | ≥80% | **below floor** |
| `convert.rs` | 15 | 6 | 21 | 71.4% | ≥80% | **below floor** |
| `error.rs` | 33 | 7 | 40 | 82.5% | ≥75% | ✓ above floor |
| `extensions/registry.rs` | 11 | 3 | 14 | 78.6% | — | typical |
| `extensions/catalog.rs` | 1 | 1 | 2 | 50.0% | — | low-volume; one survivor is high-impact |
| `evaluator/builtins/money.rs` | — | — | — | — | — | pending baseline |
| `evaluator/builtins/dates.rs` | — | — | — | — | — | pending baseline |
| `parser.rs` | — | — | — | — | ≥85% | pending baseline |
| `lexer.rs` | — | — | — | — | — | pending baseline |
| `evaluator/core.rs` | — | — | — | — | ≥85% | pending baseline |
| `prepare_host.rs` | — | — | — | — | — | pending baseline |

## How to use this document

1. **Pick a row.** Start with files below their kill-rate floor (highest-impact).
2. **Write the kill test** — referencing the spec section that motivates the behavior. Or, if the mutant is genuinely equivalent (no observable behavior change), add it to `.cargo/mutants.toml` `skip_calls` with a one-line justification.
3. **Re-run** `make mutants-<file>` to verify the survivor is killed (or skipped).
4. **Update** the per-file row in `conformance/mutation-baseline.jsonl` with the new sha and kill rate.
5. **Close** the row in this document by linking the test commit sha or skip-entry justification.

Phase 2 success criterion: every row in this document is closed (killed, equivalent-justified, or accepted-with-rationale). Until then, the baseline is incomplete and Phase 2 cannot close.
