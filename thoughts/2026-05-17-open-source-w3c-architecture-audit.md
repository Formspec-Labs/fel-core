# fel-core open-source W3C-style architecture audit - 2026-05-17

## Scope

This audit reviews `fel-core` as if it were an open-source reference
implementation and W3C-style specification project. It covers the local working
tree, including uncommitted locale-formatting changes that predate this audit.
It does not certify the project as W3C-endorsed. `docs/SPEC.md` correctly says
FEL is Formspec-internal and not a W3C Recommendation.

External review criteria used:

- W3C public-review guidance: status sections, public feedback, implementation
  experience, test suites, and open-source contribution surfaces:
  https://www.w3.org/standards/review/
- W3C Process 2025: implementation experience should demonstrate that each
  feature is implemented and interoperable:
  https://www.w3.org/policies/process/#implementation-experience
- W3C Security and Privacy Questionnaire: specifications should include
  security and privacy considerations and should revisit them as designs change:
  https://www.w3.org/TR/security-privacy-questionnaire/
- W3C Internationalization Best Practices: use an i18n self-review checklist,
  especially when adding natural-language, locale, date, or formatting behavior:
  https://www.w3.org/TR/international-specs/
- W3C TAG workmode: design review expects an explainer plus security/privacy
  self-review context:
  https://tag.w3.org/workmode/

## Verdict

`fel-core` is architecturally close to a credible open-source specification
project: it has a normative semantics document, grammar, conformance corpus,
manifest digest, security and privacy sections, resource budgets, fuzzing,
license policy, security reporting, contribution guidance, and CI wiring.

Initial audit finding: the working tree was not release-ready because
`formatNumber` and `formatDate` were implemented but not synchronized across
the ratified specification set. Follow-up remediation ratified those functions
across the spec, catalog, tests, conformance corpus, manifest, rustdoc mirror,
and sibling canonical schema.

## Findings

### P0 - Current tree broke the ratification contract

Status: closed in the follow-up remediation.

Evidence:

- `src/evaluator/core.rs:1416-1418` dispatches `locale`, `formatNumber`, and
  `formatDate`.
- `src/extensions/catalog.rs:2356-2434` adds catalog entries for
  `formatNumber` and `formatDate`.
- `tests/locale_fel_functions.rs:253-310` adds passing behavior tests for both
  functions.
- `docs/SPEC.md:261` still says the `locale` category has only 3 functions:
  `locale`, `runtimeMeta`, and `pluralCategory`.
- `docs/SPEC.md:353-354` says new builtin functions may be added only when the
  catalog schema, conformance corpus, and version notes are updated together.
- `src/extensions/mod.rs:167-174` still asserts the emitted schema has 72
  functions. The current catalog emits 74.
- `conformance/manifest.json` still records 249 fixtures and SHA-256
  `b81c18a06811d8f8a49a9ec39c9f2d158094272df7d0abd13434504f60874de6`.
  The generator now emits 251 fixtures, including `formatNumber` and
  `formatDate`.

Initial verified failures:

- `cargo fmt --all -- --check` fails on `tests/locale_fel_functions.rs`.
- `cargo test --all-features` fails in
  `extensions::tests::emit_schema_has_72_functions`: expected 72 functions,
  got 74.
- `cargo test --test schema_round_trip --all-features` fails because generated
  schema differs from the canonical schema.
- `make check-ratification` fails because generated conformance differs from
  `conformance/fel-conformance.jsonl`.

Resolution: `formatNumber` and `formatDate` remain in FEL 1.0. The fix updated
the spec category count and i18n text, canonical function schema, conformance
corpus, manifest, rustdoc mirror, schema-count test, and formatting.

### P1 - Locale formatting needs an i18n decision before it becomes normative

The existing i18n text says locale-sensitive behavior is limited to `locale()`
and `pluralCategory()` (`docs/SPEC.md:381-387`). The new implementation formats
numbers and dates with a small hand-written language split in
`src/evaluator/builtins/locale.rs`, not a declared CLDR/ICU/Intl contract.

That is risky for a W3C-style project because locale formatting is visible,
culturally variable behavior. Conformance fixtures would freeze the subset as
normative unless the spec says it is intentionally small, implementation-defined,
or host-provided.

Status: closed by the `docs/SPEC.md` internationalization section. FEL owns
only a small deterministic formatting subset; broader CLDR/ICU behavior remains
a host or extension responsibility.

### P1 - Implementation-report evidence is not yet public enough

The project has a good local corpus and an optional cross-runtime gate:

- `conformance/README.md` defines the JSONL evaluator protocol.
- `Makefile:61-78` defines Python and WASM differential checks, with
  `make ratify-external` as the external gate.
- `.github/workflows/doc.yml:71-101` runs external conformance only on schedule
  or dispatch, and only when `FORMSPEC_REPO_TOKEN` is available.

That is useful internal evidence, but it is not a W3C-style implementation
report. A reader cannot tell which conformance classes pass in Rust, Python,
WASM/TypeScript, which features are not implemented, which runtimes are public,
or whether independent implementers exist.

Status: closed by
[`conformance/IMPLEMENTATION-REPORT.md`](../conformance/IMPLEMENTATION-REPORT.md),
which maps parser, evaluator, diagnostics, and host-environment classes to
implementations and command evidence.

### P2 - The spec status is honest, but feedback routing is thin

`docs/SPEC.md:21-35` states that FEL is Formspec-internal, ratified as of
2026-05-17, and not W3C-reviewed or endorsed. That is good. A W3C-style status
section should also tell reviewers where to send feedback and where issues are
tracked. The repository metadata and `CONTRIBUTING.md` imply GitHub, but the
spec document itself does not route comments.

Status: follow-up quality item, not a ratification blocker.

### P2 - Catalog examples and generated conformance should not drift silently

`src/bin/emit-conformance-fixtures.rs:89-121` converts catalog examples into
fixtures by evaluating each example against a default `MapEnvironment`. It does
not compare the result to `Example.result_json`, nor can examples supply
environment context. This makes locale examples awkward: `locale()` can be a
catalog example with `"en-US"` but produce a conformance fixture with `null`
under the default environment.

Status: follow-up quality item, not a ratification blocker. The ratified
`formatNumber` and `formatDate` examples do not depend on host context.

## Strengths

- Specification posture is unusually clear for a young crate:
  `docs/SPEC.md`, `specs/fel/fel-grammar.md`, `conformance/manifest.json`, and
  `conformance/fel-conformance.jsonl` form a coherent specification set.
- The conformance manifest records line count and digest, and
  `scripts/check-ratification.py` verifies generated corpus parity.
- The evaluator has explicit resource controls: parser depth, evaluation depth,
  step budget, allocation budget, and deadline budget.
- Security and privacy sections exist and are specific to FEL's trust boundary:
  pure evaluation, host-supplied environment minimization, and diagnostics/trace
  privacy.
- OSS readiness is mostly present: Apache-2.0 license, `LICENSING.md`,
  `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `deny.toml`, and a CI
  workflow with lint, dependency policy, ratification, packaging, fuzz replay,
  and optional external conformance.
- `cargo deny check` passes in this working tree: advisories, bans, licenses,
  and sources are all OK.
- `rg "\bunsafe\b" src tests fuzz` found no Rust `unsafe` usage.

## Verification ledger

Commands run from `fel-core/`:

| Command | Result | Notes |
|---|---:|---|
| `git status --short` | dirty | Pre-existing changes in locale builtins, catalog, core dispatch, and tests. |
| `cargo test --test locale_fel_functions --all-features` | pass | 38 locale tests pass. |
| `cargo fmt --all -- --check` | fail | Formatting drift in `tests/locale_fel_functions.rs`. |
| `cargo test --test schema_round_trip --all-features` | fail | Generated builtin schema differs from canonical schema. |
| `make check-ratification` | fail | Generated conformance corpus differs from committed corpus. |
| `cargo test --all-features` | fail | Schema function-count test expects 72, current catalog emits 74. |
| `cargo deny check` | pass | Dependency advisories/license/source policy OK. |
| `wc -l conformance/fel-conformance.jsonl` | 249 | Matches current manifest, not current generator. |
| `shasum -a 256 conformance/fel-conformance.jsonl` | pass | Digest matches manifest value above. |
| `cargo run --features proptest-strategies --bin emit-conformance-fixtures -- 200` | drift | Emits 251 fixtures including `formatNumber` and `formatDate`. |

Follow-up verification after remediation:

| Command | Result | Notes |
|---|---:|---|
| `make ci` | pass | Includes rustfmt, clippy, cargo-deny, ratification, full tests, rustdoc broken-link denial, and package verification. |
| `make ratify-external` | pass | Python and WASM/TypeScript differential oracle passed in this checkout. |

## Recommended next move

The blocker found by this audit was resolved by ratifying locale formatting as
a full FEL 1.0 feature. Remaining non-blocking polish is to add explicit
feedback routing to the spec status section and to evolve the implementation
report when independent third-party evidence exists.
