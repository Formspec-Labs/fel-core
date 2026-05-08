# FEL Conformance Corpus

Cross-runtime differential-testing fixtures for the Formspec Expression Language.
Third-party FEL evaluators consume `fel-conformance.jsonl` to verify evaluation
semantics match the reference Rust runtime.

## Format

JSONL — one test case per line. Each line is a JSON object:

| Field | Type | Description |
|---|---|---|
| `expression` | string | FEL source to evaluate |
| `environment` | object | Flat field bindings (`{"field": <JSON value>}`); empty object when no bindings needed |
| `expectedValue` | JSON value | Canonical runtime result after `fel_to_json` |
| `expectedDiagnosticKinds` | string[] | Diagnostic variant names produced during evaluation (`"UndefinedFunction"`, `"TypeMismatch"`) |

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

## Regeneration

```sh
make conformance
```

Run from `fel-core/`. Requires `--features proptest-strategies`. The generator is
deterministic — identical output on every run.
