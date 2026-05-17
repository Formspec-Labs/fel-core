# fel-core — backlog status

All backlog rows from the 2026-05-06 audit, 2026-05-07 chaos initiative (C1-C10), and 2026-05-08 multi-lens review (R21-R41) are **closed**. See [`COMPLETED.md`](COMPLETED.md) for full narratives.

No open ratification blockers remain. Last ratified baseline includes
`formatNumber` / `formatDate` (2026-05-17).
`formatNumber` and `formatDate` are documented in `docs/SPEC.md`, covered by
`tests/locale_fel_functions.rs`, and included in `conformance/fel-conformance.jsonl`
via catalog examples. Run `make check-ratification` after changing the corpus.

The 2026-05-17 W3C-style audit follow-ups are tracked below. See
[`thoughts/2026-05-17-open-source-w3c-architecture-audit.md`](thoughts/2026-05-17-open-source-w3c-architecture-audit.md).

## Open audit findings

- **FEL-OSS-AUDIT-001** - **Closed** (2026-05-17). Locale formatting builtins
  ratified in spec, catalog, tests, and conformance corpus.
- **FEL-OSS-AUDIT-002** - **Closed** (2026-05-17). Conformance classes and
  implementation evidence are summarized in
  [`conformance/IMPLEMENTATION-REPORT.md`](conformance/IMPLEMENTATION-REPORT.md).
- **FEL-OSS-AUDIT-003** - **Closed** (2026-05-17). Locale-formatting i18n scope,
  fallback behavior, supported subset, timezone boundary, and fixture posture
  are documented in [`docs/SPEC.md`](docs/SPEC.md).

The 2026-05-17 internal-ratification pass retired the stale monorepo-audit
markers:

- **FEL-DEPRECATED-EVALUATE-001** — no deprecated evaluate-family wrappers
  remain in `src/`; `evaluate()` and `evaluate_with()` are the live entry
  points.
- **FEL-BACKWARD-ALIAS-001** — `fel_to_json()` remains a compatibility alias for
  the pre-1.0 Rust API, not normative FEL language surface. The normative value
  wire behavior is specified in [`docs/SPEC.md`](docs/SPEC.md) and covered by
  conformance fixtures.
- **FEL-DIFFERENTIAL-ORACLE-001** — the cross-runtime oracle remains ignored in
  ordinary `cargo test` because it requires sibling Python and WASM runtimes.
  The ratification posture is explicit: `make ratify` is the hermetic local
  gate, and `make ratify-external` is the implementation-report gate.

## Code smell audit (2026-05-17)

Epic **`fs-aui0`** — validated findings in [`code-smell-audit.md`](code-smell-audit.md). Priority 4 chores (DOC-001, H-006, L-001–L-025 except blocked L-017/L-024, M-013, M-014) are **closed** on `main`. High/medium refactors (H-001–H-005, etc.) remain open on the epic.

Internal-ratification artifacts:

- [`docs/SPEC.md`](docs/SPEC.md)
- [`specs/fel/fel-grammar.md`](specs/fel/fel-grammar.md)
- [`conformance/manifest.json`](conformance/manifest.json)
- [`conformance/fel-conformance.jsonl`](conformance/fel-conformance.jsonl)
- `make ratify`
- `make ratify-external` for sibling implementation-report evidence

---

## Execution protocol

For future behavioral changes:

1. **Red** — Test fails and proves the gap.
2. **Green** — Smallest fix.
3. **Refactor** — Cleanup, same behavior.
4. **Verify** — Focused tests, then full fel-core suite.

Guardrails: one behavioral change per cycle; never ship without a test that demonstrated the bug or regression risk.
