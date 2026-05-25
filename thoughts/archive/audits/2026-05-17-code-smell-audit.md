# fel-core code smell audit (2026-05-17)

Six **code-scout** passes over `fel-core/` on HEAD. Findings are ticketed under epic `fs-aui0` (`FEL-SMELL-*` in `tk`). This file is the human rollup; **`TODO.md`** carries the synced backlog section.

## Validation notes (scout, 2026-05-17)

| Topic | Correction |
|-------|------------|
| **H-006** | Array index path is **not** a live underflow: `access_path` rejects `idx == 0` and OOB before `idx - 1`. Optional hardening only. |
| **Line refs** | Stale `file:line` anchors in docs were replaced with module paths (see `docs/SPEC.md`). |
| **Catalog** | `extensions/catalog.rs` was ~2703 lines; split into `extensions/catalog/{aggregate,string,...}.rs` assembled via `LazyLock`. |
| **Fuzz corpus** | `evaluator_edge_cases.rs` no longer embeds thousands of generated tests; see `tests/corpus/fuzz_regression.jsonl`. |

## Priority 4 — closed in this pass

| Ref | Summary |
|-----|---------|
| DOC-001 | This audit file + `TODO.md` smell section |
| H-006 | `arr.get(idx - 1)` + 1-based index comment |
| L-001 … L-016, L-018 … L-025 | Low chores (lexer, strings, deps, catalog, locale, snapshots, tests, docs, deny, gitignore, parser, cache, helpers, fuzz seed) |
| M-013 | `Makefile` respects `RUSTUP_HOME` |
| M-014 | `FORMSPEC_ENGINE_PATH` for `fel-wasm-eval.mjs` |

**Post-review (formspec-scout):** `CallArgCache` keeps `args_ptr` identity (not arity-only); `fn_format` stays two-phase (`{n}` then `%s`) for correct semantics; trace tests cover `contains` + nested `formatNumber`. `pick_key` snake_case aliases are covered in `context_json` unit tests (`builds_env_from_snake_case_context_keys`). H-006 index guards have `test_index_zero_*` / `test_index_out_of_bounds_*` in `evaluator_tests.rs`.

## Scout validation (2026-05-17)

Four parallel **formspec-scout** passes on the closed P4 batch → **PASS WITH NOTES** (no reopen list).

| Area | Result |
|------|--------|
| Evaluator / trace / builtins | DONE — 93 targeted tests green |
| Catalog / deny / gitignore | DONE — 74 builtins, `cargo deny` licenses OK |
| Lexer / parser / docs / scripts | PASS — `sumWhere` / array literals intact |
| `tk` vs code | 28/29 P4 closed; only `fs-hd03` (L-017) correctly open |

**Waived closes (documented in `tk` notes, not reopened):** `fs-991y` (pointer `CallArgCache`, not index-only); `fs-tzxb` (two-phase `format`, not single-pass).

**Nit closure (commits `98a2820`, `44f1cb3`):** `eval_budget_with_extensions`; index 0/OOB tests; `[[test]] fel_proptest` + `proptest-strategies`; `FORMSPEC_ROOT` in Makefile help + `conformance/README.md`.

## Still open (not in Priority 4 batch)

| Ref | Ticket | Blocker |
|-----|--------|---------|
| L-017 | `fs-hd03` | `fs-w2ao` (split `apply_binary`) |

High/medium refactors (H-001–H-005, H-009, H-012, M-001–M-011, …) remain on epic `fs-aui0`.

## Cross-cutting

| Pattern | Where | Status |
|---------|-------|--------|
| Path segment rendering | `PathSegment::append_to_path` | H-010 closed |
| Extension arity | `ExtensionRegistry::call` | M-004 closed |
| Fuzz regression contract | `fuzz_to_regression.py` ↔ `emit-fuzz-regression-corpus` ↔ `evaluator_edge_cases` | L-025 closed |
