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
| L-001 … L-025 | Low chores (lexer, strings, deps, catalog, locale, snapshots, tests, docs, deny, gitignore, parser, cache, helpers) |
| M-013 | `Makefile` respects `RUSTUP_HOME` |
| M-014 | `FORMSPEC_ENGINE_PATH` for `fel-wasm-eval.mjs` |

## Still open (not in Priority 4 batch)

High/medium refactors (H-001–H-005, H-009, H-012, M-001–M-011, L-017 blocked on H-005, L-024) remain on the epic until picked up.

## Cross-cutting

| Pattern | Where | Status |
|---------|-------|--------|
| Path segment rendering | `PathSegment::append_to_path` | H-010 closed |
| Extension arity | `ExtensionRegistry::call` | M-004 closed |
| Fuzz regression contract | `fuzz_to_regression.py` ↔ `emit-fuzz-regression-corpus` ↔ `evaluator_edge_cases` | L-025 closed |
