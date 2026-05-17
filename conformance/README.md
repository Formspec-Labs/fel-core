# FEL Conformance Corpus

Cross-runtime differential-testing fixtures for the Formspec Expression Language.
Third-party FEL evaluators consume `fel-conformance.jsonl` to verify evaluation
semantics match the reference Rust runtime.

This corpus is part of the FEL 1.0 internal-ratification specification set.
The manifest in [`manifest.json`](manifest.json) records the expected line count
and SHA-256 digest for the public corpus.

Implementation evidence is tracked in
[`IMPLEMENTATION-REPORT.md`](IMPLEMENTATION-REPORT.md).

## Format

JSONL — one test case per line. Each line is a JSON object:

| Field | Type | Description |
|---|---|---|
| `expression` | string | FEL source to evaluate |
| `environment` | object | Flat field bindings (`{"field": <JSON value>}`); empty object when no bindings needed |
| `expectedValue` | JSON value | Canonical runtime result after `fel_to_json` |
| `expectedDiagnosticKinds` | string[] | Diagnostic variant names produced during evaluation (`"UndefinedFunction"`, `"TypeMismatch"`, `"ArityMismatch"`) |

Every fixture uses this public schema. Stale development-only fixture shapes such
as `{ "source": ..., "value": ... }` are not ratification artifacts.

`expectedValue` uses public/result JSON, not the typed wire encoding. FEL
numbers are decimal values: JSON numbers appear only when the canonical encoder
can emit stable JSON number text and safe whole integers; otherwise expected
numeric results appear as normalized decimal strings. Native JavaScript
`BigInt` is not part of this corpus because JSON has no `BigInt` value.

## Third-party evaluator protocol

1. Parse `expression` as FEL.
2. Load `environment` entries into your evaluation scope.
3. Evaluate and convert the result to JSON.
4. Assert the JSON result deep-equals `expectedValue`.
5. Assert the diagnostic kinds produced match `expectedDiagnosticKinds`.

Key semantics encoded in the corpus:

- **Null propagation** — most operators propagate null to their result (equality is the exception).
- **Equality** — `null == null` is true; `null == <anything>` is false.
- **Short-circuit** — `and`/`or` skip the right operand when the left determines the result.
- **Date arithmetic** — `dateAdd` / `dateDiff` with days/months/years.
- **Money operations** — `moneyAdd`, currency matching, money × scalar.
- **Type coercion** — explicit casts; implicit coercion where the spec requires it.

## Cross-runtime differential (WASM)

The Rust↔WASM oracle uses `scripts/fel-wasm-eval.mjs`. Point it at a built
`formspec-engine` dist tree when not using the default monorepo sibling layout:

```sh
export FORMSPEC_ENGINE_PATH=/path/to/formspec
make test-differential-wasm
```

## Regeneration

```sh
make conformance
```

Run from `fel-core/`. Requires `--features proptest-strategies`. The generator is
deterministic — identical output on every run.

The ratification gate verifies this claim:

```sh
make check-ratification
```

For the full local internal-ratification gate, run:

```sh
make ratify
```

For optional cross-runtime implementation evidence, run:

```sh
make ratify-external
```
