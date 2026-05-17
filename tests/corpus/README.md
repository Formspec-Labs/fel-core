# Fuzz regression corpus (`fuzz_regression.jsonl`)

One JSON object per line, consumed by `fuzz_regression_corpus` in
`tests/evaluator_edge_cases.rs`.

| Field | Meaning |
|-------|---------|
| `id` | 12-char SHA-256 prefix (stable row identity) |
| `expression` | FEL source |
| `mustParse` | `true` → parse failure is a regression; `false` → parse must fail |
| `displayOracle` | Optional `Display` of evaluated value when `mustParse` is true |

## Maintenance

- **New fuzz finds:** `make fuzz-extract` (appends via `scripts/fuzz_to_regression.py`).
- **Refresh oracles** after parser, evaluator, or `Value::Display` changes:
  `make fuzz-regression-refresh` (re-runs `emit-fuzz-regression-corpus` over the file).
- **One-shot migration** from legacy `fuzz_regression_*` tests:
  `scripts/archive/extract_fuzz_corpus_from_tests.py` (historical only).
