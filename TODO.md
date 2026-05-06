# fel-core — open backlog

Audit seed: 2026-05-06 multi-agent review. Resolved work is archived in [`COMPLETED.md`](COMPLETED.md).

---

## Quick reference

| ID | Topic | Priority | Status |
|----|-------|----------|--------|
| 6 | Split `extensions` monolith (registry / catalog / schema) | P0 | open |
| 7 | Builtin dispatch (`eval_function`) vs catalog drift | P0 | partial |
| 8 | Date ↔ JSON round-trip vs “no silent coercion” policy | P1 | policy review |
| 10 | `Diagnostic` + AST source spans | P1 | open |
| 11 | `Expr::VarRef` vs overloaded `FieldRef` | P1 | open |
| 13 | Normalize builtin diagnostics (type / arity messages) | P2 | partial |
| 14 | Deduplicate builtin helpers (compare, money fold, dates, …) | P2 | partial |
| 15 | `Value::Object` lookup + duplicate keys outside FEL literals | P3 | open |
| 30 | `Money.currency` fixed-size / `Copy` newtype | P3 | open |
| 38 | Property tests + fuzz smoke (`proptest` in dev-deps, unused) | P3 | partial |

---

## Recommended order (large refactors)

Mechanical / low coupling first; behavior-changing API milestones last.

1. **#6** — File split only (no behavior change): `extensions/{mod,catalog,schema,registry,types}.rs` → run `schema_round_trip`, `builtin_catalog_consistency`, full `cargo test`.
2. **#14** then **#13** — Shared helpers while preserving current diagnostics, then unify messages (updates tests that assumed silent null on mismatch).
3. **#7** — Stronger `builtin_catalog_consistency` and/or table dispatch *after* #6 so merge conflicts stay localized.
4. **#11** — Semver-major `Expr` variant; touches parser, printer, `dependencies`, evaluator.
5. **#10** — Optional scaffolding (`Diagnostic::span: Option<…>`) early; full evaluator spans need AST spans + `diag` threading (often after #11).
6. **#15 / #30** — Representation / API releases when profiling or semver budget allows.
7. **#8** — Reopen only if product requires JSON round-trip for `Date`; today documented as intentional (see item body).
8. **#38** — Add harness; builtin-heavy properties after #13 to avoid churn.

---

## Execution protocol (TDD loop)

For each behavioral change:

1. **Red** — Test fails and proves the gap.
2. **Green** — Smallest fix.
3. **Refactor** — Cleanup, same behavior.
4. **Verify** — Focused tests, then full fel-core suite.

Guardrails: one behavioral change per cycle; never ship without a test that demonstrated the bug or regression risk.

---

## P0 — Architecture & dispatch

### 6. `extensions.rs` monolith `[open]`

**Size:** ~2,966 lines (`wc -l`). ~2,096 lines are the `BUILTIN_FUNCTIONS` slice alone; registry, schema emitters, and types share one file.

**Goal:** Split into focused modules (e.g. `catalog.rs`, `schema.rs`, `registry.rs`, optional `types.rs`) behind `pub mod extensions` with stable `fel_core::extensions::*` paths.

**Verify:** `cargo test`, `tests/schema_round_trip.rs`, `tests/builtin_catalog_consistency.rs`, `cargo run -p fel-core --bin emit-fel-schema` if your pipeline uses it.

---

### 7. `eval_function` match vs builtin catalog `[partial]`

**Where:** `src/evaluator/core.rs` — `fn eval_function` ~`1020–1162` (builtin match ~`1021–1160`; line ranges drift with edits).

**Problem:** Adding a builtin touches the match, `evaluator/builtins/*.rs`, and `BUILTIN_FUNCTIONS` in extensions. No compile-time lockstep. `tests/builtin_catalog_consistency.rs` covers catalog names calling without `undefined function`, but parse-skipped names can hide gaps.

**Directions:** Table/macro dispatch *or* tighten tests (explicit allowlist for names not callable as `ident()`). **`is_eager_traceable_function`** (`evaluator/util.rs`) is not the full builtin list — only eager trace whitelist; do not merge blindly with the catalog.

---

## P1 — Policy, diagnostics model, AST

### 8. Date values vs JSON round-trip `[policy review]`

**Where:** `src/convert.rs` — dates serialize as ISO strings; `json_to_fel` does not coerce plain strings to `Date` (documented “no silent date coercion”; tests e.g. `string_no_date_coercion`).

**Conclusion:** Not a bug unless product mandates round-trip. **If** hosts need it: mirror Money with explicit `$type` (or opt-in / major version — wire shape change).

---

### 10. `Diagnostic` lacks source spans `[open]`

**Where:** `src/error.rs` (`Diagnostic`); evaluator uses `diag()` with strings only. AST (`src/ast.rs`) has no span fields — lexer/parser use `SpannedToken` during parse, then spans are not stored on `Expr`.

**Goal:** `span: Option<Range<usize>>` on `Diagnostic`; extend `fel_diagnostics_to_json_value*` when non-`None`. Full evaluator-quality locations require spans on expressions or evaluation frames (larger than parser-only errors).

---

### 11. Bare identifiers vs `$` field refs `[open]`

**Where:** Parser — bare `Identifier` → `FieldRef` ~`551–562` (approx.; verify in tree); `$…` → `parse_field_ref`. Evaluator — `eval_field_ref` ~`445–526`, `PostfixAccess` + `FieldRef` merge ~`408–438`.

**Problem:** Both use `Expr::FieldRef`. Printer always prints `FieldRef` with a leading `$`, so source fidelity is lost for bare names.

**Goal:** `Expr::VarRef` (or equivalent) for bare identifiers; `FieldRef` only when `$` is present. **Breaking change** for exhaustive `match` on public `Expr`.

---

## P2 — Builtin internals

### 13. Inconsistent builtin diagnostics `[partial]`

Mixed patterns: some type mismatches silent → Null (`builtins/strings.rs`, `moneyAmount` arm in `evaluator/core.rs` ~`1116+`), others explicit `diag`. Only structured `DiagnosticKind` today is undefined-function; `dates.rs` has a rare **warning**.

**Goal:** Central helpers on `Evaluator` (e.g. `require_arg_count`, typed `require_*`) so messages include function name and received shape before widening `DiagnosticKind`.

---

### 14. Duplication across builtins `[partial]`

Hotspots (approximate; re-verify when editing):

| Pattern | Example locations |
|---------|-------------------|
| Ordered min/max across discriminant | `builtins/aggregates.rs` (variadic + aggregate + minWhere + maxWhere) |
| Money fold + currency check | `builtins/money.rs`, `aggregates.rs` moneySumWhere |
| Date-or-string → Date | `builtins/dates.rs` (several fns) |
| Predicate `let` loops | `aggregates.rs` count/every/some vs `filter_where` in `core.rs` |
| Arity checks | scattered vs centralized patterns |

**Goal:** `compare_values`, `sum_money_values`, shared predicate iteration, `require_arg_count` — see refactor notes in scout pass; preserve behavior in the first extraction pass.

---

## P3 — Types, perf, tests

### 15. `Object(Vec<(String, Value)>)` lookup `[open]`

**Where:** `src/types.rs`; `MapEnvironment` / `access_path` in `evaluator/core.rs` use linear search per segment.

**Note:** FEL `{ … }` literals reject duplicate keys at parse time. Duplicates remain possible for JSON→FEL or programmatic `Value::Object` construction. `serde_json` `preserve_order` addresses JSON map ordering, not in-memory lookup cost.

**Direction:** `indexmap` (or equivalent) on `Value::Object` + `.get` in hot paths — separate semver/API decision from #32 (already landed).

---

### 30. `Money.currency` as `String` `[open]`

ISO 4217 codes fit three ASCII letters; heap `String` per value. **Direction:** `CurrencyCode([u8; 3])` + `Display` / `TryFrom` — breaking API change for field access patterns.

---

### 38. Property-based testing `[partial]`

`proptest` is listed in `Cargo.toml` dev-dependencies but unused in source tests today.

**Ideas:** parse↔print, `convert` JSON round-trip, null propagation — after **#13** if assertions depend on diagnostic behavior. Parser smoke / random input last (panic hunting).

---
