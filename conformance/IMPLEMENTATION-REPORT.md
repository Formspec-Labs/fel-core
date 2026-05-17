# FEL 1.0 Implementation Report - 2026-05-17

## Status

This is a Formspec-internal implementation report for the FEL 1.0 conformance
classes. It is not a W3C implementation report and does not imply W3C review or
endorsement.

Local evidence collected on 2026-05-17:

- `make ci` passed.
- `make ratify-external` passed against the available sibling Python and
  WASM/TypeScript wrappers.

## Corpus

- Corpus: [`fel-conformance.jsonl`](fel-conformance.jsonl)
- Manifest: [`manifest.json`](manifest.json)
- Fixture count: 251
- SHA-256:
  `6a7c63e36b72f899dcc5280b16c135e5c714c56dab96bd6994fd3beac001ba70`

The corpus includes catalog-derived examples for `formatNumber()` and
`formatDate()` so the locale-formatting subset is executable conformance
surface, not documentation-only prose.

## Conformance Classes

| Class | Rust reference | Python wrapper | WASM/TypeScript wrapper | Evidence |
|---|---|---|---|---|
| Parser | Implemented by `src/lexer.rs`, `src/parser.rs`, and parser tests. | Covered by `make ratify-external` when the sibling Python runtime is available. | Covered by `make ratify-external` when the sibling WASM runtime is available. | `make ci`; `make ratify-external`. |
| Evaluator | Implemented by `src/evaluator/` and public conformance tests. | Covered by Python differential oracle. | Covered by WASM differential oracle. | `tests/public_conformance_corpus.rs`; `make ratify-external`. |
| Diagnostics | Implemented by `src/error.rs` and diagnostic JSON tests. | Expected to preserve diagnostic-kind names from corpus. | Expected to preserve diagnostic-kind names from corpus. | `expectedDiagnosticKinds` in public corpus. |
| Host environment | Implemented by `Environment`, `MapEnvironment`, and `FormspecEnvironment`. | Covered when sibling runtime context adapters are present. | Covered when sibling WASM bridge/context adapters are present. | `tests/environment_integration_tests.rs`; external gate. |

## Commands

Local hermetic gate:

```sh
make ratify
```

OSS-readiness gate:

```sh
make ci
```

Optional cross-runtime implementation gate:

```sh
make ratify-external
```

`make ratify-external` requires sibling Python and WASM runtime setup. It passed
locally on 2026-05-17 in this checkout. The GitHub workflow also runs the
external gate on schedule or dispatch when maintainers configure the private
sibling repository token.

## Known Boundaries

- The public corpus is the portable FEL 1.0 contract. It is intentionally
  smaller than the full internal test suite.
- `formatNumber()` and `formatDate()` define a deterministic locale subset for
  conformance. They do not claim full CLDR/ICU coverage.
- Independent third-party implementation evidence is not yet available. Current
  implementation evidence is reference Rust plus maintained sibling runtime
  wrappers.
