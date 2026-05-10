# fel-core — backlog status

All backlog rows from the 2026-05-06 audit, 2026-05-07 chaos initiative (C1–C10), and 2026-05-08 multi-lens review (R21–R41) are **closed**. See [`COMPLETED.md`](COMPLETED.md) for full narratives.

No open items remain — except the debt markers surfaced by the 2026-05-08 monorepo audit below.

### Monorepo audit — untracked debt

- **FEL-DEPRECATED-EVALUATE-001 — Remove `#[deprecated]` evaluate-family functions** `[4 / 2 / 3]` (12)
  - Seven `#[deprecated(since = "0.1.0")]` wrappers in `src/evaluator/core.rs:283-379` (evaluate, evaluate_with_env, evaluate_single, etc.) redirect to `evaluate_with`. No removal date set. Downstream consumers (formspec, WOS) migrated to `evaluate_with` — verify zero call-sites remain, then delete the deprecated surface in a minor bump.
  - **Done when:** `grep -r '#\[deprecated' src/` returns zero hits in the evaluator module; `cargo test --workspace` green.

- **FEL-BACKWARD-ALIAS-001 — Remove backward-compat `fel_to_ui_json` alias** `[3 / 1 / 2]` (6)
  - `src/convert.rs:248` carries a `Backward-compatible alias for UI-friendly encoding` redirect. Low surface, but every alias is a name the codebase must carry forever. Confirm no external consumer, then inline or delete.
  - **Done when:** alias removed; all callers use the canonical name.

- **FEL-DIFFERENTIAL-ORACLE-001 — Decision on `#[ignore]` differential oracle test** `[3 / 1 / 2]` (6)
  - `tests/differential_oracle.rs` is `#[ignore]` by default; requires `make test-differential` to enable. Decide: promote to CI-gated nightly, or document the invariant it guards and close.
  - **Done when:** test runs in CI (nightly label or similar) or is closed with documented rationale.

---

## Execution protocol

For future behavioral changes:

1. **Red** — Test fails and proves the gap.
2. **Green** — Smallest fix.
3. **Refactor** — Cleanup, same behavior.
4. **Verify** — Focused tests, then full fel-core suite.

Guardrails: one behavioral change per cycle; never ship without a test that demonstrated the bug or regression risk.
