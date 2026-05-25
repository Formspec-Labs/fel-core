# fel-core — backlog status

All 2026-05 audit/review backlog rows are closed; see [`COMPLETED.md`](COMPLETED.md) for narratives. Ratified baseline is current (`make ratify` is the local gate; `make ratify-external` runs cross-runtime parity).

## Open work

| Surface | Where tracked | Status |
|---|---|---|
| Mutation-survivor follow-ups | [`thoughts/2026-05-23-mutation-survivor-followups.md`](thoughts/2026-05-23-mutation-survivor-followups.md) | active — FUTs 1-17 closed; one strict-equivalent residual (`prepare_host.rs:289:25`) documented |
| Code-smell epic `fs-aui0` | `tk` (project ticket system) — 16 open H/M refactors + L-017 (`fs-hd03`, blocked on `fs-w2ao`) | active |
| `lib.rs` re-export coverage gate | [`tests/lib_reexport_coverage.toml`](tests/lib_reexport_coverage.toml) — enforced every PR | active |

## Conventions

- **Workflow** — see [`CONTRIBUTING.md`](CONTRIBUTING.md) for red-green-refactor and gate requirements.
- **History** — closed rows live in [`COMPLETED.md`](COMPLETED.md); shipped plans + closed audits in [`thoughts/archive/`](thoughts/archive/).
- **Spec** — normative semantics in [`docs/SPEC.md`](docs/SPEC.md) and [`specs/fel/fel-grammar.md`](specs/fel/fel-grammar.md); change those before code.
