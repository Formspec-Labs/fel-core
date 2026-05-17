# Archived fuzz tooling

`extract_fuzz_corpus_from_tests.py` — one-shot migration from generated
`fuzz_regression_*` `#[test]` functions to seed JSONL. The committed corpus lives in
`tests/corpus/fuzz_regression.jsonl`; use `make fuzz-regression-refresh` for updates.
