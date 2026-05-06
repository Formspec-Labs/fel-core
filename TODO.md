# fel-core — open backlog

Audit seed: 2026-05-06 multi-agent review. Resolved work is archived in [`COMPLETED.md`](COMPLETED.md).

**Stack integration (2026-05-06):** Items **#7, #11, #13, #14, #15, #30**, **`fel_proptest`** suite, **`Diagnostic::span` + JSON**, and **`wos-core` `EvalContext` IndexMap** objects landed in-repo. Remaining open items are called out below.

---

## Quick reference

| ID | Topic | Priority | Status |
|----|-------|----------|--------|
| 7 | Builtin dispatch (`eval_function`) vs catalog drift | P0 | **done** |
| 8 | Date ↔ JSON round-trip vs “no silent coercion” policy | P1 | policy review |
| 10 | `Diagnostic` + AST source spans | P1 | **partial** — see §10 |
| 11 | `Expr::VarRef` vs overloaded `FieldRef` | P1 | **done** |
| 13 | Normalize builtin diagnostics (type / arity messages) | P2 | **done** |
| 14 | Deduplicate builtin helpers (compare, money fold, dates, …) | P2 | **done** |
| 15 | `Value::Object` lookup + duplicate keys outside FEL literals | P3 | **done** |
| 30 | `Money.currency` fixed-size / `Copy` newtype | P3 | **done** |
| 38 | Property tests + fuzz smoke | P3 | **partial** — see §38 |

---

## Recommended order (large refactors)

Mechanical / low coupling first; behavior-changing API milestones last.

1. ~~**#14** then **#13**~~ — Done (helpers + unified diagnostics; tests updated).
2. ~~**#7**~~ — Done (catalog consistency test no longer silently skips parse failures; allowlist reserved for intentional non-`ident()` names).
3. ~~**#11**~~ — Done (`Expr::VarRef`; downstream formspec-core / wos-lint updated).
4. **#10** — **Partial:** `Diagnostic::span` + JSON wire landed in `error.rs`. Parser token spans on parse errors and evaluator `diag` threading / `Expr` spans remain optional follow-ups.
5. ~~**#15 / #30**~~ — Done (`IndexMap` object storage; `CurrencyCode`; semver-breaking for external exhaustive matches).
6. **#8** — Reopen only if product requires JSON round-trip for `Date`; today documented as intentional (see item body).
7. **#38** — **Partial:** `tests/fel_proptest.rs` added (parse/print, null, JSON round-trip). **`tests/civil_calendar_proptest.rs`** unchanged. Random-input parser fuzz / panic hunting still optional (last per prior guidance).

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

### 7. `eval_function` match vs builtin catalog `[done]`

**Where:** `src/evaluator/core.rs` — `fn eval_function`. Catalog: `src/extensions/catalog.rs`. Test: `tests/builtin_catalog_consistency.rs`.

**Landed:** Catalog entries that should parse as `ident()` must parse — failures surface instead of being skipped. Explicit allowlist remains for names that are intentionally not invoked as `ident()` calls. **`is_eager_traceable_function`** (`evaluator/util.rs`) stays separate from the full builtin set.

---

## P1 — Policy, diagnostics model, AST

### 8. Date values vs JSON round-trip `[policy review]`

**Where:** `src/convert.rs` — dates serialize as ISO strings; `json_to_fel` does not coerce plain strings to `Date` (documented “no silent date coercion”; tests e.g. `string_no_date_coercion`).

**Conclusion:** Not a bug unless product mandates round-trip. **If** hosts need it: mirror Money with explicit `$type` (or opt-in / major version — wire shape change).

---

### 10. `Diagnostic` lacks source spans `[partial]`

**Where:** `src/error.rs` — `Diagnostic` includes optional **`span: Option<Range<usize>>`**; **`fel_diagnostics_to_json_value*`** emits `span: { start, end }` when set. Unit tests cover JSON shape.

**Still open (optional):** Thread lexer offsets into **parse** errors first; later AST / evaluation frames for evaluator diagnostics. **`kind`** on JSON wire — coordinate with hosts if needed.

**Where AST:** `src/ast.rs` still has no span fields — lexer uses `SpannedToken` during parse only.

---

### 11. Bare identifiers vs `$` field refs `[done]`

**Where:** Parser builds **`Expr::VarRef`** for bare identifiers; **`Expr::FieldRef`** for `$…`. Printer preserves surface form. Evaluator and `dependencies.rs` handle both. Downstream: formspec-core `fel_analysis` / `fel_condition_group_lift`, wos-lint `fel_analysis`.

**Breaking:** Exhaustive `match` on `Expr` must include `VarRef`.

---

## P2 — Builtin internals

### 13. Inconsistent builtin diagnostics `[done]`

**Landed:** `reject_expected_type` / arity helpers extended to **`fn_round`**, **`fn_power`**, casts in **`logic_types`** (`number` / `boolean` / `date`, **`if`** condition typing), **`selected`** (non-array → diagnostic + null). **`if`** uses exact arity where applicable.

**Optional later:** `DiagnosticKind::TypeMismatch` once messages are stable.

---

### 14. Duplication across builtins `[done]`

**Landed:** Shared helpers in **`evaluator/builtins/helpers.rs`** and **`Evaluator`** (`require_min_args`, **`require_exact_args`**, etc.); migrated arity sites including **`if`**. Further duplication cleanup is incremental only.

---

## P3 — Types, perf, tests

### 15. `Object` storage and lookup `[done]`

**Where:** `src/types.rs` — **`Value::Object(IndexMap<String, Value>)`**; hot paths use map lookup. **Breaking** for code matching `Value::Object` structurally.

**Note:** FEL `{ … }` literals still reject duplicate keys at parse time. JSON / programmatic construction may still produce duplicates — policy unchanged.

---

### 30. `Money.currency` newtype `[done]`

**Landed:** **`CurrencyCode([u8; 3])`** with `parse`, `as_str`, **`Display`**. **`Money { currency: CurrencyCode }`**. JSON convert and builtins updated.

---

### 38. Property-based testing `[partial]`

**Landed:** **`tests/fel_proptest.rs`** — parse↔print (integer literals), null / equality behavior, JSON scalar + shallow object round-trip via `json_to_fel` / `fel_to_json`.

**Still optional:** Parser random-input fuzz (panic hunting) after diagnostic-heavy suites stabilize; calendar suite remains **`tests/civil_calendar_proptest.rs`** only.

---
